mod banks;
pub mod marginfi_accounts;
mod mints;

use mints::MintsCache;
use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Result};
use log::{info, trace};
use marginfi::state::{marginfi_account::MarginfiAccount, marginfi_group::Bank};
use solana_program::clock::Clock;
use solana_sdk::pubkey::Pubkey;

use anchor_lang::AccountDeserialize;

use crate::{
    cache::{banks::BanksCache, marginfi_accounts::MarginfiAccountsCache},
    common::{get_marginfi_message_type, MessageType},
    comms::CommsClient,
    config::Config,
};

// TODO: not completely sure that this trait is really needed.
pub trait CacheEntry {
    fn address(&self) -> Pubkey;
}

pub struct Cache {
    pub clock: RwLock<Clock>,
    pub marginfi_accounts: MarginfiAccountsCache,
    pub banks: BanksCache,
    pub mints: MintsCache,
}

impl Cache {
    pub fn new(clock: Clock) -> Self {
        Self {
            clock: RwLock::new(clock),
            marginfi_accounts: MarginfiAccountsCache::default(),
            banks: BanksCache::default(),
            mints: MintsCache::default(),
        }
    }

    pub fn update_clock(&self, clock: Clock) -> Result<()> {
        trace!("Updating Clock in cache: {:?}", clock);
        *self
            .clock
            .write()
            .map_err(|e| anyhow!("Failed to lock Clock for the update: {}", e))? = clock;
        Ok(())
    }

    pub fn get_clock(&self) -> Result<Clock> {
        Ok(self
            .clock
            .read()
            .map_err(|e| anyhow!("Failed to lock Clock for reading: {}", e))?
            .clone())
    }
}

//TODO: consider moving out to it's own module if it grows larger
pub struct CacheLoader<T: CommsClient> {
    program_id: Pubkey,
    cache: Arc<Cache>,
    comms_client: T,
}

impl<T: CommsClient> CacheLoader<T> {
    pub fn new(config: &Config, cache: Arc<Cache>) -> Result<Self> {
        let comms_client = T::new(config)?;
        Ok(Self {
            program_id: config.marginfi_program_id,
            comms_client,
            cache,
        })
    }

    pub fn load_cache(&self) -> Result<()> {
        // Load Marginfi account and banks
        self.load_accounts()?;
        self.load_mints()?;
        Ok(())
    }

    pub fn load_accounts(&self) -> Result<()> {
        info!("Loading accounts for the program id {}...", self.program_id);

        let slot = self.cache.get_clock()?.slot;

        let accounts = self.comms_client.get_program_accounts(&self.program_id)?;
        let mut marginfi_accounts_count = 0;
        let mut banks_count = 0;
        for (address, account) in accounts {
            match get_marginfi_message_type(&account.data) {
                Some(MessageType::MarginfiAccount) => {
                    let marginfi_account: MarginfiAccount =
                        MarginfiAccount::try_deserialize(&mut account.data.as_slice())?;
                    self.cache
                        .marginfi_accounts
                        .update(slot, address, &marginfi_account)?;
                    marginfi_accounts_count += 1;
                }
                Some(MessageType::Bank) => {
                    let bank: Bank = Bank::try_deserialize(&mut account.data.as_slice())?;
                    self.cache.banks.update(slot, address, &bank)?;
                    banks_count += 1;
                }
                _ => {
                    // Not yet
                }
            }
        }

        info!(
            "Loaded {} Marginfi accounts and {} Banks.",
            marginfi_accounts_count, banks_count
        );

        Ok(())
    }

    pub fn load_mints(&self) -> Result<()> {
        info!("Loading mints...");

        let mint_addresses = self.cache.banks.get_all_mints()?;

        let mut mints_counter = 0;
        for (address, mint) in self.comms_client.get_accounts(&mint_addresses)? {
            self.cache.mints.update(address, &mint)?;
            mints_counter += 1;
        }

        info!("Loaded {} mints", mints_counter);
        Ok(())
    }
}

#[cfg(test)]
pub mod test_util {
    use std::time::SystemTime;

    use solana_program::clock::Clock;
    use solana_sdk::clock::UnixTimestamp;

    pub fn generate_test_clock(slot: u64) -> Clock {
        let current_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as UnixTimestamp;

        solana_program::clock::Clock {
            slot,
            epoch_start_timestamp: current_timestamp - 3600, // 1 hour ago
            epoch: 0,
            leader_schedule_epoch: 1,
            unix_timestamp: current_timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::generate_test_clock;
    use crate::comms::test_util::MockedCommsClient;
    use solana_sdk::pubkey::Pubkey;
    use std::sync::Arc;

    use super::*;

    #[test]
    fn test_cache_new() {
        let clock = generate_test_clock(1);
        let cache = Cache::new(clock);
        assert_eq!(cache.get_clock().unwrap().slot, 1);
    }

    #[test]
    fn test_cache_update_clock() {
        let initial_clock = generate_test_clock(1);
        let cache = Cache::new(initial_clock);

        // Create a new clock with different values
        let mut updated_clock = generate_test_clock(2);
        updated_clock.epoch = 2;

        // Update the cache with the new clock
        cache.update_clock(updated_clock.clone()).unwrap();

        // Verify the cache now holds the updated clock
        let cached_clock = cache.get_clock().unwrap();
        assert_eq!(cached_clock.slot, 2);
        assert_eq!(cached_clock.epoch, 2);
        assert_eq!(cached_clock.unix_timestamp, updated_clock.unix_timestamp);
    }

    #[test]
    fn test_cache_loader_new() {
        // Prepare dummy config and cache
        let config = Config {
            marginfi_program_id: Pubkey::new_unique(),
            // add other required fields with dummy values
            ..Default::default()
        };
        let cache = Arc::new(Cache::new(generate_test_clock(1)));

        // Try to create a CacheLoader using the mocked comms client
        let loader = CacheLoader::<MockedCommsClient>::new(&config, cache.clone());
        assert!(loader.is_ok());
        let loader = loader.unwrap();
        assert_eq!(loader.program_id, config.marginfi_program_id);
    }

    //TODO: add the CacheLoader tests after figuring out how to serialize MarginfiAccount.
}
