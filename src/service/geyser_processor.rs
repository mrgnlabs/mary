use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crossbeam::channel::Receiver;
use log::{debug, error, info};
use solana_sdk::{account::Account, clock::Clock};

use crate::{
    cache::Cache,
    service::geyser_subscriber::{GeyserMessage, GeyserMessageType},
};

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
        GeyserProcessor {
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
                    if let Err(err) = self.process_message(msg) {
                        error!("Failed to process Geyser message: {}", err);
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

    fn process_message(&self, msg: GeyserMessage) -> anyhow::Result<()> {
        debug!("Processing Geyser message: {:?}", msg);
        match msg.message_type {
            GeyserMessageType::ClockUpdate => {
                update_solana_clock(&self.cache, &msg.account)?;
            }
            _ => {
                // Not yet
            }
        }
        Ok(())
    }
}

fn update_solana_clock(cache: &Arc<Cache>, account: &Account) -> anyhow::Result<()> {
    let clock: Clock = bincode::deserialize::<Clock>(&account.data)?;
    cache.update_clock(clock)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use crate::cache::test_util::generate_test_clock;

    use super::*;

    #[test]
    fn test_update_solana_clock_success() {
        let clock = generate_test_clock(1);
        let cache = Arc::new(Cache::new(generate_test_clock(2)));

        let account = Account {
            lamports: 0,
            data: bincode::serialize(&clock).unwrap(),
            owner: solana_sdk::pubkey::Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
        };

        let clock: Clock = bincode::deserialize::<Clock>(&account.data).unwrap();

        update_solana_clock(&cache, &account).unwrap();

        let result = cache.update_clock(clock.clone());
        assert!(result.is_ok());

        let cached_clock = cache.get_clock().unwrap();
        assert_eq!(cached_clock, clock);
    }

    #[test]
    fn test_update_solana_clock_invalid_data() {
        let cache = Arc::new(Cache::new(generate_test_clock(1)));
        let account = Account {
            lamports: 0,
            data: vec![1, 2, 3, 4], // Invalid data for Clock
            owner: solana_sdk::pubkey::new_rand(),
            executable: false,
            rent_epoch: 0,
        };

        let result = update_solana_clock(&cache, &account);
        assert!(result.is_err());
    }
}
