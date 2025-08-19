mod marginfi_accounts;

use std::sync::RwLock;

use anyhow::{anyhow, Result};
use solana_program::clock::Clock;

use crate::cache::marginfi_accounts::MarginfiAccountsCache;

pub struct Cache {
    pub clock: RwLock<Clock>,
    pub marginfi_accounts: MarginfiAccountsCache,
}

impl Cache {
    pub fn new(clock: Clock) -> Self {
        Self {
            clock: RwLock::new(clock),
            marginfi_accounts: MarginfiAccountsCache::default(),
        }
    }

    pub fn update_clock(&self, clock: Clock) -> Result<()> {
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
}
