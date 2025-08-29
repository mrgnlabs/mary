use std::fmt;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::common::{get_marginfi_message_type, MessageType};
use crate::{cache::Cache, config::Config};
use anyhow::{anyhow, Result};
use crossbeam::channel::Sender;
use futures::stream::StreamExt; // Brings `next` into scope for streams
use log::{error, info, trace};
use solana_sdk::sysvar;
use solana_sdk::{account::Account, clock::Clock, pubkey::Pubkey};
use tokio::runtime::{Builder, Runtime};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::geyser::{
    subscribe_update, SubscribeUpdate, SubscribeUpdateAccountInfo,
};
use yellowstone_grpc_proto::{geyser::SubscribeRequestFilterAccounts, prelude::SubscribeRequest};

const SOLANA_CLOCK_BYTES: [u8; 32] = sysvar::clock::id().to_bytes();

#[derive(Debug)]
pub struct GeyserMessage {
    pub(crate) message_type: MessageType,
    pub(crate) slot: u64,
    pub(crate) address: Pubkey,
    pub(crate) account: Account,
}

impl GeyserMessage {
    pub fn new(
        message_type: MessageType,
        slot: u64,
        geyser_update_account: SubscribeUpdateAccountInfo,
    ) -> Result<Self> {
        let address = Pubkey::try_from(geyser_update_account.pubkey.clone())
            .map_err(|err| anyhow!("Invalid Address in {:?}: {:?}", geyser_update_account, err))?;

        let owner = Pubkey::try_from(geyser_update_account.owner.clone())
            .map_err(|err| anyhow!("Invalid Owner in {:?}: {:?}", geyser_update_account, err))?;

        Ok(GeyserMessage {
            message_type,
            slot,
            address,
            account: Account {
                lamports: geyser_update_account.lamports,
                data: geyser_update_account.data,
                owner,
                executable: geyser_update_account.executable,
                rent_epoch: geyser_update_account.rent_epoch,
            },
        })
    }
}

impl fmt::Display for GeyserMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[type: {:?}, slot: {}, address: {}]",
            self.message_type, self.slot, self.address,
        )
    }
}

pub struct GeyserSubscriber {
    endpoint: String,
    x_token: String,
    stop: Arc<AtomicBool>,
    tls_config: ClientTlsConfig,
    subscribe_req: SubscribeRequest,
    tokio_rt: Runtime,
    cache: Arc<Cache>,
    marginfi_program_id_bytes: [u8; 32],
    geyser_tx: Sender<GeyserMessage>,
}

impl GeyserSubscriber {
    pub fn new(
        config: &Config,
        stop: Arc<AtomicBool>,
        cache: Arc<Cache>,
        geyser_tx: Sender<GeyserMessage>,
    ) -> Result<Self> {
        let tls_config = ClientTlsConfig::new().with_native_roots();

        let marginfi_program_id = config.marginfi_program_id;
        let subscribe_req = build_geyser_subscribe_request(marginfi_program_id)?;

        let tokio_rt = Builder::new_multi_thread()
            .thread_name("GeyserService")
            .worker_threads(2)
            .enable_all()
            .build()?;

        Ok(Self {
            endpoint: config.geyser_endpoint.clone(),
            x_token: config.geyser_x_token.clone(),
            stop,
            tls_config,
            subscribe_req,
            tokio_rt,
            cache,
            marginfi_program_id_bytes: config.marginfi_program_id.to_bytes(),
            geyser_tx,
        })
    }

    pub fn run(&self) -> Result<()> {
        info!("Entering the GeyserService loop.");
        while !self.stop.load(Ordering::Relaxed) {
            info!("Connecting to Geyser...");

            let mut client = self.tokio_rt.block_on(
                GeyserGrpcClient::build_from_shared(self.endpoint.clone())?
                    .x_token(Some(self.x_token.clone()))?
                    .tls_config(self.tls_config.clone())?
                    .connect(),
            )?;

            let (_, mut stream) = self
                .tokio_rt
                .block_on(client.subscribe_with_request(Some(self.subscribe_req.clone())))?;

            while let Some(msg) = self.tokio_rt.block_on(stream.next()) {
                match msg {
                    Ok(event) => {
                        if let Err(e) = handle_event(
                            &self.marginfi_program_id_bytes,
                            &self.cache.get_clock()?,
                            &self.geyser_tx,
                            &event,
                        ) {
                            error!("Error handling Geyser update {:?}: {}", event, e);
                        }
                    }
                    Err(e) => {
                        error!("Received error from Geyser: {}", e);
                        break;
                    }
                }

                // Breaking the loop on stop request
                if self.stop.load(Ordering::Relaxed) {
                    break;
                }
            }
        }
        info!("The GeyserService loop is stopped.");

        Ok(())
    }
}

fn build_geyser_subscribe_request(marginfi_program_id: Pubkey) -> Result<SubscribeRequest> {
    let mut account_filters: HashMap<String, SubscribeRequestFilterAccounts> = HashMap::new();

    let clock_filter = SubscribeRequestFilterAccounts {
        account: vec![sysvar::clock::id().to_string()],
        ..Default::default()
    };
    account_filters.insert("SolanaClock".to_string(), clock_filter);

    let marginfi_program_filter = SubscribeRequestFilterAccounts {
        owner: vec![marginfi_program_id.to_string()],
        ..Default::default()
    };
    account_filters.insert("MarginfiProgram".to_string(), marginfi_program_filter);

    Ok(SubscribeRequest {
        accounts: account_filters,
        ..Default::default()
    })
}

fn handle_event(
    marginfi_program_id_bytes: &[u8; 32], //TODO: come up with better way to use it in the method without passing
    clock: &Clock,
    geyser_tx: &Sender<GeyserMessage>,
    event: &SubscribeUpdate,
) -> Result<()> {
    match &event.update_oneof {
        Some(subscribe_update::UpdateOneof::Account(subscribe_account))
            if subscribe_account.slot >= clock.slot =>
        {
            if let Some(account) = &subscribe_account.account {
                if account.owner == marginfi_program_id_bytes {
                    trace!("Handling Marginfi update: {:?}", event);
                    if let Some(message_type) = get_marginfi_message_type(&account.data) {
                        let msg = GeyserMessage::new(
                            message_type,
                            subscribe_account.slot,
                            account.clone(),
                        )?;
                        geyser_tx.send(msg)?;
                    }
                } else if account.pubkey == SOLANA_CLOCK_BYTES {
                    trace!("Handling Solana clock update: {:?}", event);
                    let msg = GeyserMessage::new(
                        MessageType::Clock,
                        subscribe_account.slot,
                        account.clone(),
                    )?;
                    geyser_tx.send(msg)?;
                }
            }
        }
        _ => {
            trace!("Handling Geyser update: {:?}", event);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crossbeam::channel;
    use yellowstone_grpc_proto::geyser::SubscribeUpdateAccount;

    use crate::cache::test_util::generate_test_clock;

    use super::*;

    static MARGINFI_PROGRAM_ID_BYTES: [u8; 32] = [1u8; 32];

    fn make_account_info(pubkey: Pubkey) -> SubscribeUpdateAccountInfo {
        SubscribeUpdateAccountInfo {
            pubkey: pubkey.to_bytes().to_vec(),
            owner: pubkey.to_bytes().to_vec(),
            lamports: 42,
            data: vec![1, 2, 3],
            executable: false,
            rent_epoch: 0,
            write_version: 1,
            txn_signature: None,
        }
    }

    #[test]
    fn test_handle_event_clock_update() {
        let (tx, rx) = channel::unbounded();
        let clock = generate_test_clock(1);

        let account_info = make_account_info(sysvar::clock::id());

        let subscribe_account = SubscribeUpdateAccount {
            slot: 10,
            account: Some(account_info.clone()),
            is_startup: false,
        };

        let event = SubscribeUpdate {
            update_oneof: Some(subscribe_update::UpdateOneof::Account(subscribe_account)),
            ..Default::default()
        };

        let result = handle_event(&MARGINFI_PROGRAM_ID_BYTES, &clock, &tx, &event);
        assert!(result.is_ok());

        // Should have sent a message
        let msg = rx.try_recv().expect("Should have received a message");
        assert!(matches!(msg.message_type, MessageType::Clock));
        assert_eq!(msg.slot, 10);
        assert_eq!(msg.address, sysvar::clock::id());
        assert_eq!(msg.account.lamports, 42);
    }

    #[test]
    fn test_handle_event_non_clock_account() {
        let (tx, rx) = channel::unbounded();
        let clock = generate_test_clock(1);

        let random_pubkey = Pubkey::new_unique();
        let account_info = make_account_info(random_pubkey);
        let subscribe_account = SubscribeUpdateAccount {
            slot: 10,
            account: Some(account_info),
            is_startup: false,
        };

        let event = SubscribeUpdate {
            update_oneof: Some(subscribe_update::UpdateOneof::Account(subscribe_account)),
            ..Default::default()
        };

        let result = handle_event(&MARGINFI_PROGRAM_ID_BYTES, &clock, &tx, &event);
        assert!(result.is_ok());

        // Should NOT have sent a message
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_handle_event_slot_too_low() {
        let (tx, rx) = channel::unbounded();
        let clock = generate_test_clock(2);

        let account_info = make_account_info(sysvar::clock::id());

        let subscribe_account = SubscribeUpdateAccount {
            slot: 1,
            account: Some(account_info),
            is_startup: false,
        };

        let event = SubscribeUpdate {
            update_oneof: Some(subscribe_update::UpdateOneof::Account(subscribe_account)),
            ..Default::default()
        };

        let result = handle_event(&MARGINFI_PROGRAM_ID_BYTES, &clock, &tx, &event);
        assert!(result.is_ok());

        // Should NOT have sent a message
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_handle_event_no_account() {
        let clock = generate_test_clock(1);
        let (tx, rx) = channel::unbounded();
        let subscribe_account = SubscribeUpdateAccount {
            slot: 10,
            account: None,
            is_startup: false,
        };

        let event = SubscribeUpdate {
            update_oneof: Some(subscribe_update::UpdateOneof::Account(subscribe_account)),
            ..Default::default()
        };

        let result = handle_event(&MARGINFI_PROGRAM_ID_BYTES, &clock, &tx, &event);
        assert!(result.is_ok());

        // Should NOT have sent a message
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_handle_event_other_update_type() {
        let (tx, rx) = channel::unbounded();
        let clock = generate_test_clock(1);
        let event = SubscribeUpdate {
            update_oneof: None,
            ..Default::default()
        };

        let result = handle_event(&MARGINFI_PROGRAM_ID_BYTES, &clock, &tx, &event);
        assert!(result.is_ok());

        // Should NOT have sent a message
        assert!(rx.try_recv().is_err());
    }
}
