use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::{cache::Cache, config::Config};
use anyhow::{anyhow, Result};
use futures::stream::StreamExt; // Brings `next` into scope for streams
use log::{debug, error, info, trace};
use solana_sdk::{clock::Clock, pubkey::Pubkey, sysvar};
use tokio::runtime::{Builder, Runtime};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::{
    geyser::{subscribe_update, SubscribeRequestFilterAccounts, SubscribeUpdate},
    prelude::SubscribeRequest,
};

pub struct GeyserSubscriber {
    endpoint: String,
    x_token: String,
    stop: Arc<AtomicBool>,
    tls_config: ClientTlsConfig,
    subscribe_req: SubscribeRequest,
    tokio_rt: Runtime,
    cache: Arc<Cache>,
}

impl GeyserSubscriber {
    pub fn new(config: &Config, stop: Arc<AtomicBool>, cache: Arc<Cache>) -> Result<Self> {
        let tracked_accounts = get_tracked_accounts();

        let tls_config = ClientTlsConfig::new().with_native_roots();

        let subscribe_req = build_subscribe_request(&tracked_accounts)?;

        let tokio_rt = Builder::new_multi_thread()
            .thread_name("GeyserService")
            .worker_threads(2)
            .enable_all()
            .build()?;

        Ok(GeyserSubscriber {
            endpoint: config.geyser_endpoint.clone(),
            x_token: config.geyser_x_token.clone(),
            stop,
            tls_config,
            subscribe_req,
            tokio_rt,
            cache,
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
                        if let Err(e) = handle_event(&self.cache.get_clock()?, &event) {
                            error!("Error handling Geyser message {:?}: {}", event, e);
                        }
                    }
                    Err(e) => {
                        error!("Received error message from Geyser: {}", e);
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

fn get_tracked_accounts() -> Vec<Pubkey> {
    // Placeholder for actual logic to get tracked accounts
    vec![sysvar::clock::id()]
}

fn build_subscribe_request(tracked_accounts: &[Pubkey]) -> Result<SubscribeRequest> {
    // Accounts
    let accounts = SubscribeRequestFilterAccounts {
        account: tracked_accounts.iter().map(|a| a.to_string()).collect(),
        ..Default::default()
    };

    let request = SubscribeRequest {
        accounts: HashMap::from([("Accounts".to_string(), accounts)]),
        ..Default::default()
    };

    // Program

    Ok(request)
}

pub fn handle_event(clock: &Clock, event: &SubscribeUpdate) -> Result<()> {
    trace!("Handling Geyser message: {:?}", event);

    match &event.update_oneof {
        Some(subscribe_update::UpdateOneof::Account(subscribe_account))
            if subscribe_account.slot >= clock.slot =>
        {
            if let Some(account) = &subscribe_account.account {
                // TODO: is it possible to evaluate without cloning?
                let address = Pubkey::try_from(account.pubkey.clone()).map_err(|err| {
                    anyhow!("Invalid Pubkey in Geyser message {:?}: {:?}", event, err)
                })?;

                if address == sysvar::clock::id() {
                    debug!("Received Solana clock update: {:?}", event);
                }
            }
        }
        _ => {}
    }

    Ok(())
}
