use std::{collections::HashMap, sync::RwLock};

use anyhow::{anyhow, Result};
use log::trace;
use marginfi::state::{
    marginfi_group::{Bank, BankConfig},
    price::OracleSetup,
};
use solana_sdk::pubkey::Pubkey;

use crate::cache::CacheEntry;

#[derive(Debug)]
pub struct CachedBank {
    pub slot: u64,
    pub address: Pubkey,
    pub mint: Pubkey,
    pub _mint_decimals: u8,
    pub _group: Pubkey,
    pub _oracle_type: OracleSetup,
    pub _oracle_accounts: Vec<Pubkey>,
    // TODO: add pub asset_tag: ???,
    //emode config
}

impl CacheEntry for CachedBank {}

impl CachedBank {
    pub fn from(slot: u64, address: Pubkey, bank: &Bank) -> Self {
        Self {
            slot,
            address,
            mint: bank.mint,
            _mint_decimals: bank.mint_decimals,
            _group: bank.group,
            _oracle_type: bank.config.oracle_setup,
            _oracle_accounts: get_oracle_accounts(&bank.config),
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
            .map_err(|e| anyhow!("Failed to lock the banks cache for update! {}", e))?;

        if banks
            .get(&address)
            .map_or(true, |existing| existing.slot < upd_cached_bank.slot)
        {
            trace!("Updating the Bank in cache: {:?}", upd_cached_bank);
            banks.insert(address, upd_cached_bank);
        }

        Ok(())
    }

    pub fn get_all_mints(&self) -> Result<Vec<Pubkey>> {
        Ok(self
            .banks
            .read()
            .map_err(|e| anyhow!("Failed to lock the banks cache for reading mints: {}", e))?
            .values()
            .map(|bank| bank.mint)
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
    use marginfi::state::marginfi_group::{Bank, BankConfig};
    use marginfi::state::price::OracleSetup;
    use solana_sdk::pubkey::Pubkey;

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
}

#[cfg(test)]
mod tests {
    use super::test_util::create_bank_with_oracles;
    use super::*;
    use marginfi::state::marginfi_group::BankConfig;
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
        assert_eq!(cached.address, address);
        assert_eq!(cached.mint, bank.mint);
        assert_eq!(cached._mint_decimals, bank.mint_decimals);
        assert_eq!(cached._group, bank.group);
        assert_eq!(cached._oracle_type, bank.config.oracle_setup);
        assert_eq!(cached._oracle_accounts, vec![oracle1, oracle2]);
    }

    #[test]
    fn test_cache_entry_trait() {
        let slot = 42;
        let address = Pubkey::new_unique();
        let bank = create_bank_with_oracles(vec![]);
        let cached = CachedBank::from(slot, address, &bank);

        assert_eq!(cached.slot, slot);
        assert_eq!(cached.address, address);
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
        assert_eq!(cached.address, address);
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
        let mints = cache.get_all_mints().unwrap();
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

        let mut mints = cache.get_all_mints().unwrap();
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

        let result = cache.get_all_mints();
        assert!(result.is_err());
    }
}
