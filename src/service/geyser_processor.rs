use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crossbeam::channel::Receiver;
use log::{error, info, trace};
use marginfi::state::{marginfi_account::MarginfiAccount, marginfi_group::Bank};
use solana_sdk::{account::Account, clock::Clock};
// Add the trait import for try_deserialize (adjust if you use a different crate)
use anchor_lang::AccountDeserialize;

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
            GeyserMessageType::ClockUpdate => {
                process_clock_update(&self.cache, &msg.account)?;
            }
            GeyserMessageType::MarginfiAccountUpdate => {
                process_marginfi_account_update(&self.cache, msg)?;
            }
            GeyserMessageType::MarginfiBankUpdate => {
                process_marginfi_bank_update(&self.cache, msg)?;
            }
            _ => {
                // Not yet
            }
        }
        Ok(())
    }

    pub fn queue_depth(&self) -> usize {
        self.geyser_rx.len()
    }
}

fn process_clock_update(cache: &Arc<Cache>, account: &Account) -> anyhow::Result<()> {
    let clock: Clock = bincode::deserialize::<Clock>(&account.data)?;
    cache.update_clock(clock)?;
    Ok(())
}

fn process_marginfi_account_update(cache: &Arc<Cache>, msg: &GeyserMessage) -> anyhow::Result<()> {
    let marginfi_account: MarginfiAccount =
        MarginfiAccount::try_deserialize(&mut msg.account.data.as_slice())?;
    cache
        .marginfi_accounts
        .update(msg.slot, msg.address, &marginfi_account)?;
    Ok(())
}

fn process_marginfi_bank_update(cache: &Arc<Cache>, msg: &GeyserMessage) -> anyhow::Result<()> {
    let bank: Bank = Bank::try_deserialize(&mut msg.account.data.as_slice())?;
    cache.banks.update(msg.slot, msg.address, &bank)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use solana_sdk::pubkey::Pubkey;

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

        process_clock_update(&cache, &account).unwrap();

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
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
        };

        let result = process_clock_update(&cache, &account);
        assert!(result.is_err());
    }
}
