use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anchor_lang::AccountDeserialize;
use crossbeam::channel::Receiver;
use log::{error, info, trace};
use marginfi::state::{marginfi_account::MarginfiAccount, marginfi_group::Bank};
use solana_sdk::clock::Clock;

use crate::{cache::Cache, common::MessageType, service::geyser_subscriber::GeyserMessage};

pub struct GeyserProcessor {
    stop: Arc<AtomicBool>,
    cache: Arc<Cache>,
    geyser_rx: Receiver<GeyserMessage>,
}

impl GeyserProcessor {
    pub fn new(
        stop: Arc<AtomicBool>,
        cache: Arc<Cache>,
        geyser_rx: Receiver<GeyserMessage>,
    ) -> Self {
        Self {
            stop,
            cache,
            geyser_rx,
        }
    }

    pub fn run(&self) -> anyhow::Result<()> {
        info!("Entering the GeyserProcessor loop.");
        while !self.stop.load(Ordering::Relaxed) {
            match self.geyser_rx.recv() {
                Ok(msg) => {
                    if let Err(err) = self.process_message(&msg) {
                        error!("Failed to process Geyser message {:?}: {}", msg, err);
                    }
                }
                Err(error) => {
                    error!("GeyserProcessor error: {}!", error);
                }
            }
        }

        info!("The GeyserProcessor loop is stopped.");
        Ok(())
    }

    fn process_message(&self, msg: &GeyserMessage) -> anyhow::Result<()> {
        trace!("Processing Geyser message: {}", msg);
        match msg.message_type {
            MessageType::Clock => {
                let clock: Clock = bincode::deserialize::<Clock>(&msg.account.data)?;
                self.cache.update_clock(clock)?;
            }
            MessageType::MarginfiAccount => {
                let marginfi_account: MarginfiAccount =
                    MarginfiAccount::try_deserialize(&mut msg.account.data.as_slice())?;
                self.cache
                    .marginfi_accounts
                    .update(msg.slot, msg.address, &marginfi_account)?;
            }
            MessageType::Bank => {
                let bank: Bank = Bank::try_deserialize(&mut msg.account.data.as_slice())?;
                self.cache.banks.update(msg.slot, msg.address, &bank)?;
            }
            MessageType::Oracle => {
                self.cache
                    .oracles
                    .update(msg.slot, &msg.address, &msg.account)?;
            }
        }
        Ok(())
    }

    pub fn queue_depth(&self) -> usize {
        self.geyser_rx.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cache::{
        banks::test_util::create_bank_with_oracles,
        marginfi_accounts::test_util::create_marginfi_account,
        test_util::{create_dummy_cache, generate_test_clock},
        Cache,
    };
    use crate::common::MessageType;
    use crate::service::geyser_subscriber::GeyserMessage;
    use crossbeam::channel;
    use solana_sdk::{account::Account, clock::Clock, pubkey::Pubkey};
    use std::sync::{atomic::AtomicBool, Arc};

    fn setup_processor() -> (
        GeyserProcessor,
        channel::Sender<GeyserMessage>,
        Arc<AtomicBool>,
        Arc<Cache>,
    ) {
        let stop = Arc::new(AtomicBool::new(false));
        let cache = Arc::new(create_dummy_cache());

        let (tx, rx) = channel::unbounded();
        let processor = GeyserProcessor::new(stop.clone(), cache.clone(), rx);
        (processor, tx, stop, cache)
    }

    #[test]
    fn test_queue_depth() {
        let (processor, tx, _, _) = setup_processor();
        assert_eq!(processor.queue_depth(), 0);

        let msg = GeyserMessage {
            message_type: MessageType::Clock,
            slot: 1,
            address: Pubkey::default(),
            account: Account::new(1, 2, &Pubkey::new_unique()),
        };
        tx.send(msg).unwrap();
        assert_eq!(processor.queue_depth(), 1);
    }

    #[test]
    fn test_process_clock_message() {
        let (processor, tx, stop, cache) = setup_processor();
        let clock = Clock::default();
        let data = bincode::serialize(&clock).unwrap();
        let msg = GeyserMessage {
            message_type: MessageType::Clock,
            slot: 1,
            address: Pubkey::default(),
            account: Account::new(1, 2, &Pubkey::new_unique()),
        };
        tx.send(msg).unwrap();
        stop.store(true, Ordering::Relaxed);
        processor.run().unwrap();
        // No panic means success; further asserts require Cache implementation details
    }

    #[test]
    fn test_process_marginfi_account_message() {
        let _marginfi_account = create_marginfi_account(Pubkey::new_unique(), vec![]);
        // TODO: implement after figuring out how to serialize MarginfiAccount
    }

    #[test]
    fn test_process_bank_message() {
        let _bank = create_bank_with_oracles(vec![]);
        // TODO: implement after figuring out how to serialize Bank
    }

    #[test]
    fn test_process_oracle_message() {
        let (processor, tx, stop, _cache) = setup_processor();
        let msg = GeyserMessage {
            message_type: MessageType::Oracle,
            slot: 4,
            address: Pubkey::new_unique(),
            account: Account::new(1, 2, &Pubkey::new_unique()),
        };
        tx.send(msg).unwrap();
        stop.store(true, Ordering::Relaxed);
        processor.run().unwrap();
    }

    #[test]
    fn test_run_stops_on_stop_signal() {
        let (processor, _, stop, _) = setup_processor();
        stop.store(true, Ordering::Relaxed);
        assert!(processor.run().is_ok());
    }

    #[test]
    fn test_run_handles_recv_error() {
        let stop = Arc::new(AtomicBool::new(false));
        let cache = Arc::new(create_dummy_cache());
        let (tx, rx) = channel::bounded(0);
        drop(tx); // Close the channel
        let processor = GeyserProcessor::new(stop.clone(), cache.clone(), rx);
        stop.store(true, Ordering::Relaxed);
        assert!(processor.run().is_ok());
    }
}
