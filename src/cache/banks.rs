use std::{collections::HashMap, sync::RwLock};

use anyhow::{anyhow, Result};
use log::{debug, trace};
use marginfi_type_crate::types::{Bank, BankConfig, OracleSetup};
use solana_sdk::pubkey::Pubkey;

use crate::cache::CacheEntry;

#[derive(Debug, Clone)]
pub struct CachedBankOracle {
    pub oracle_type: OracleSetup,
    pub oracle_addresses: Vec<Pubkey>,
}

#[derive(Clone, Debug)]
pub struct CachedBank {
    pub slot: u64,
    pub _address: Pubkey,
    pub _mint_decimals: u8,
    pub mint: Pubkey,
    pub _group: Pubkey,
    pub oracle: CachedBankOracle,
    // TODO: add pub asset_tag: ???,
    //emode config
}

impl CacheEntry for CachedBank {}

impl CachedBank {
    pub fn from(slot: u64, address: Pubkey, bank: &Bank) -> Self {
        Self {
            slot,
            _address: address,
            mint: bank.mint,
            _mint_decimals: bank.mint_decimals,
            _group: bank.group,
            oracle: CachedBankOracle {
                oracle_type: bank.config.oracle_setup,
                oracle_addresses: get_oracle_accounts(&bank.config),
            },
        }
    }
}

#[derive(Default)]
pub struct BanksCache {
    banks: RwLock<HashMap<Pubkey, CachedBank>>,
}

impl BanksCache {
    pub fn update(&self, slot: u64, address: Pubkey, bank: &Bank) -> Result<()> {
        let upd_cached_bank = CachedBank::from(slot, address, bank);

        let mut banks = self
            .banks
            .write()
            .map_err(|e| anyhow!("Failed to lock the Banks cache for update! {}", e))?;

        if banks
            .get(&address)
            .map_or(true, |existing| existing.slot < upd_cached_bank.slot)
        {
            trace!("Updating the Bank in cache: {:?}", upd_cached_bank);
            banks.insert(address, upd_cached_bank);
        }

        Ok(())
    }
    
    pub fn get_mints(&self) -> Result<Vec<Pubkey>> {
        Ok(self
            .banks
            .read()
            .map_err(|e| anyhow!("Failed to lock the Banks cache for reading mints: {}", e))?
            .values()
            .map(|bank| bank.mint)
            .collect())
    }

 pub fn get_oracles_data(&self) -> Result<Vec<CachedBankOracle>> {
        Ok(self
            .banks
            .read()
            .map_err(|e| {
                anyhow!(
                    "Failed to lock the banks cache for reading oracle accounts: {}",
                    e
                )
            })?
            .values()
            .map(|bank| bank.oracle.clone())
            .collect())
    }


    pub fn get(&self, address: &Pubkey) -> anyhow::Result<CachedBank> {
        debug!("Getting bank from cache: {:?}", address);
        let cached_bank = self.banks
            .read()
            .map_err(|e| anyhow!("Failed to lock the banks cache for getting! {}", e))?
            .get(address)
            .ok_or(anyhow!("Failed to get the bank from cache"))?.clone();
        Ok(cached_bank)
    }

    pub fn get_banks_map(&self) -> anyhow::Result<HashMap<Pubkey, Bank>> {
        Ok(self
            .banks
            .read()
            .map_err(|e| {
                anyhow!(
                    "Failed to lock the banks cache for reading oracle accounts: {}",
                    e
                )
            })?
            .values()
            .map(|bank| (bank._address, Bank {})bank.oracle.clone())
            .collect())
    }
}

fn get_oracle_accounts(bank_config: &BankConfig) -> Vec<Pubkey> {
    bank_config
        .oracle_keys
        .iter()
        .filter(|key| **key != Pubkey::default())
        .copied()
        .collect()
}

#[cfg(test)]
pub mod test_util {
    use marginfi_type_crate::types::{bank::OracleSetup, Bank, BankConfig};
    use solana_sdk::pubkey::Pubkey;

    use crate::cache::banks::CachedBank;

    pub fn create_bank_with_oracles(oracles: Vec<Pubkey>) -> Bank {
        let mut keys = [Pubkey::default(); 5];
        for (i, key) in oracles.into_iter().take(5).enumerate() {
            keys[i] = key;
        }
        Bank {
            mint: Pubkey::new_unique(),
            mint_decimals: 6,
            group: Pubkey::new_unique(),
            config: BankConfig {
                oracle_setup: OracleSetup::PythPushOracle,
                oracle_keys: keys,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn create_dummy_cached_bank() -> CachedBank {
        CachedBank::from(0, Pubkey::new_unique(), &create_bank_with_oracles(vec![]))
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::create_bank_with_oracles;
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_cached_bank_from() {
        let slot = 123;
        let address = Pubkey::new_unique();
        let oracle1 = Pubkey::new_unique();
        let oracle2 = Pubkey::new_unique();
        let bank = create_bank_with_oracles(vec![oracle1, Pubkey::default(), oracle2]);
        let cached = CachedBank::from(slot, address, &bank);

        assert_eq!(cached.slot, slot);
        assert_eq!(cached._address, address);
        assert_eq!(cached.mint, bank.mint);
        assert_eq!(cached._mint_decimals, bank.mint_decimals);
        assert_eq!(cached._group, bank.group);
        assert_eq!(cached.oracle.oracle_type, bank.config.oracle_setup);
        assert_eq!(cached.oracle.oracle_addresses, vec![oracle1, oracle2]);
    }

    #[test]
    fn test_cache_entry_trait() {
        let slot = 42;
        let address = Pubkey::new_unique();
        let bank = create_bank_with_oracles(vec![]);
        let cached = CachedBank::from(slot, address, &bank);

        assert_eq!(cached.slot, slot);
        assert_eq!(cached._address, address);
    }

    #[test]
    fn test_banks_cache_update_and_retrieve() {
        let cache = BanksCache::default();
        let slot = 100;
        let address = Pubkey::new_unique();
        let bank = create_bank_with_oracles(vec![]);
        cache.update(slot, address, &bank).unwrap();

        let banks = cache.banks.read().unwrap();
        let cached = banks.get(&address).unwrap();
        assert_eq!(cached.slot, slot);
        assert_eq!(cached._address, address);
    }

    #[test]
    fn test_banks_cache_update_only_newer_slot() {
        let cache = BanksCache::default();
        let address = Pubkey::new_unique();
        let bank1 = create_bank_with_oracles(vec![]);
        let bank2 = create_bank_with_oracles(vec![]);
        // Insert with slot 10
        cache.update(10, address, &bank1).unwrap();
        // Try to update with older slot (should not update)
        cache.update(5, address, &bank2).unwrap();

        let banks = cache.banks.read().unwrap();
        let cached = banks.get(&address).unwrap();
        assert_eq!(cached.slot, 10);
    }

    #[test]
    fn test_get_oracle_accounts_filters_default() {
        let oracle1 = Pubkey::new_unique();
        let oracle2 = Pubkey::default();
        let oracle3 = Pubkey::new_unique();
        let config = BankConfig {
            oracle_keys: [
                oracle1,
                oracle2,
                oracle3,
                Pubkey::default(),
                Pubkey::default(),
            ],
            ..Default::default()
        };
        let result = get_oracle_accounts(&config);
        assert_eq!(result, vec![oracle1, oracle3]);
    }

    #[test]
    fn test_banks_cache_update_lock_error() {
        let cache = Arc::new(BanksCache::default());
        let address = Pubkey::new_unique();
        let bank = create_bank_with_oracles(vec![]);

        // Poison the lock
        {
            let cache2 = Arc::clone(&cache);
            let _ = thread::spawn(move || {
                let _lock = cache2.banks.write().unwrap();
                panic!("Poison the lock");
            })
            .join();
        }

        let result = cache.update(1, address, &bank);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_all_mints_empty() {
        let cache = BanksCache::default();
        let mints = cache.get_mints().unwrap();
        assert!(mints.is_empty());
    }

    #[test]
    fn test_get_all_mints() {
        let cache = BanksCache::default();

        let bank1 = create_bank_with_oracles(vec![]);
        let address1 = Pubkey::new_unique();
        let mint1 = bank1.mint;
        cache.update(1, address1, &bank1).unwrap();

        let bank2 = create_bank_with_oracles(vec![]);
        let address2 = Pubkey::new_unique();
        let mint2 = bank2.mint;
        cache.update(2, address2, &bank2).unwrap();

        let mut mints = cache.get_mints().unwrap();
        mints.sort();
        let mut expected = vec![mint1, mint2];
        expected.sort();
        assert_eq!(mints, expected);
    }

    #[test]
    fn test_get_all_mints_lock_error() {
        let cache = Arc::new(BanksCache::default());

        // Poison the lock
        {
            let cache2 = Arc::clone(&cache);
            let _ = thread::spawn(move || {
                let _lock = cache2.banks.write().unwrap();
                panic!("Poison the lock");
            })
            .join();
        }

        let result = cache.get_mints();
        assert!(result.is_err());
    }

    #[test]
    fn test_banks_cache_get_oracles_data() {
        let cache = BanksCache::default();
        let oracle1 = Pubkey::new_unique();
        let oracle2 = Pubkey::new_unique();
        let bank = create_bank_with_oracles(vec![oracle1, oracle2]);
        let address = Pubkey::new_unique();
        cache.update(1, address, &bank).unwrap();

        let oracles = cache.get_oracles_data().unwrap();
        assert_eq!(oracles.len(), 1);
        assert_eq!(oracles[0].oracle_addresses, vec![oracle1, oracle2]);
        assert_eq!(oracles[0].oracle_type, bank.config.oracle_setup);
    }

    #[test]
    fn test_banks_cache_get_oracles_data_empty() {
        let cache = BanksCache::default();
        let oracles = cache.get_oracles_data().unwrap();
        assert!(oracles.is_empty());
    }

    #[test]
    fn test_banks_cache_get_oracles_data_lock_error() {
        let cache = Arc::new(BanksCache::default());

        // Poison the lock
        {
            let cache2 = Arc::clone(&cache);
            let _ = thread::spawn(move || {
                let _lock = cache2.banks.write().unwrap();
                panic!("Poison the lock");
            })
            .join();
        }

        let result = cache.get_oracles_data();
        assert!(result.is_err());
    }

    #[test]
    fn test_banks_cache_update_multiple_banks() {
        let cache = BanksCache::default();
        let bank1 = create_bank_with_oracles(vec![]);
        let bank2 = create_bank_with_oracles(vec![]);
        let address1 = Pubkey::new_unique();
        let address2 = Pubkey::new_unique();

        cache.update(1, address1, &bank1).unwrap();
        cache.update(2, address2, &bank2).unwrap();

        let banks = cache.banks.read().unwrap();
        assert_eq!(banks.len(), 2);
        assert!(banks.contains_key(&address1));
        assert!(banks.contains_key(&address2));
    }

    #[test]
    fn test_banks_cache_update_same_slot_does_not_overwrite() {
        let cache = BanksCache::default();
        let address = Pubkey::new_unique();
        let bank1 = create_bank_with_oracles(vec![]);
        let bank2 = create_bank_with_oracles(vec![]);
        cache.update(10, address, &bank1).unwrap();
        cache.update(10, address, &bank2).unwrap();

        let banks = cache.banks.read().unwrap();
        let cached = banks.get(&address).unwrap();
        // Should be the last inserted bank with the same slot
        assert_eq!(cached.mint, bank1.mint);
    }

    #[test]
    fn test_banks_cache_get_oracles_data_multiple_banks() {
        let cache = BanksCache::default();

        let oracle1 = Pubkey::new_unique();
        let oracle2 = Pubkey::new_unique();
        let bank1 = create_bank_with_oracles(vec![oracle1]);
        let address1 = Pubkey::new_unique();

        let oracle3 = Pubkey::new_unique();
        let bank2 = create_bank_with_oracles(vec![oracle2, oracle3]);
        let address2 = Pubkey::new_unique();

        cache.update(1, address1, &bank1).unwrap();
        cache.update(2, address2, &bank2).unwrap();

        let mut oracles = cache.get_oracles_data().unwrap();
        oracles.sort_by_key(|o| o.oracle_addresses.first().cloned());

        assert_eq!(oracles.len(), 2);
        assert!(oracles.iter().any(|o| o.oracle_addresses == vec![oracle1]));
        assert!(oracles
            .iter()
            .any(|o| o.oracle_addresses == vec![oracle2, oracle3]));
    }

    #[test]
    fn test_banks_cache_get_oracles_data_no_oracles() {
        let cache = BanksCache::default();
        let bank = create_bank_with_oracles(vec![]);
        let address = Pubkey::new_unique();
        cache.update(1, address, &bank).unwrap();

        let oracles = cache.get_oracles_data().unwrap();
        assert_eq!(oracles.len(), 1);
        assert!(oracles[0].oracle_addresses.is_empty());
    }

    #[test]
    fn test_banks_cache_get_oracles_data_duplicate_addresses() {
        let cache = BanksCache::default();
        let oracle = Pubkey::new_unique();
        let bank = create_bank_with_oracles(vec![oracle, oracle]);
        let address = Pubkey::new_unique();
        cache.update(1, address, &bank).unwrap();

        let oracles = cache.get_oracles_data().unwrap();
        assert_eq!(oracles.len(), 1);
        assert_eq!(oracles[0].oracle_addresses, vec![oracle, oracle]);
    }

    #[test]
    fn test_banks_cache_get_oracles_data_after_update() {
        let cache = BanksCache::default();
        let oracle1 = Pubkey::new_unique();
        let bank1 = create_bank_with_oracles(vec![oracle1]);
        let address = Pubkey::new_unique();
        cache.update(1, address, &bank1).unwrap();

        let oracles = cache.get_oracles_data().unwrap();
        assert_eq!(oracles.len(), 1);
        assert_eq!(oracles[0].oracle_addresses, vec![oracle1]);

        let oracle2 = Pubkey::new_unique();
        let bank2 = create_bank_with_oracles(vec![oracle2]);
        cache.update(2, address, &bank2).unwrap();

        let oracles = cache.get_oracles_data().unwrap();
        assert_eq!(oracles.len(), 1);
        assert_eq!(oracles[0].oracle_addresses, vec![oracle2]);
    }
}
