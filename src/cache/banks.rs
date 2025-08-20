use std::{collections::HashMap, sync::RwLock};

use anyhow::anyhow;
use log::debug;
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
    pub _mint: Pubkey,
    pub _mint_decimals: u8,
    pub _group: Pubkey,
    pub _oracle_type: OracleSetup,
    pub _oracle_accounts: Vec<Pubkey>,
    // TODO: add pub asset_tag: ???,
    //emode config
}

impl CacheEntry for CachedBank {
    fn slot(&self) -> u64 {
        self.slot
    }

    fn address(&self) -> Pubkey {
        self.address
    }
}

impl CachedBank {
    pub fn from(slot: u64, address: Pubkey, bank: &Bank) -> Self {
        Self {
            slot,
            address,
            _mint: bank.mint,
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
    pub fn update(&self, slot: u64, address: Pubkey, bank: &Bank) -> anyhow::Result<()> {
        let cached_bank = CachedBank::from(slot, address, bank);
        debug!("Updating bank in cache: {:?}", cached_bank);
        self.banks
            .write()
            .map_err(|e| anyhow!("Failed to lock the banks cache for update! {}", e))?
            .insert(address, cached_bank);
        Ok(())
    }
}

fn get_oracle_accounts(bank_config: &BankConfig) -> Vec<Pubkey> {
    bank_config
        .oracle_keys
        .iter()
        .filter(|key| **key != Pubkey::default())
        .map(|key| *key)
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use marginfi::state::marginfi_group::{Bank, BankConfig};
    use marginfi::state::price::OracleSetup;
    use std::sync::Arc;
    use std::thread;

    fn create_bank_with_oracles(oracles: Vec<Pubkey>) -> Bank {
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
        assert_eq!(cached._mint, bank.mint);
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

        assert_eq!(cached.slot(), slot);
        assert_eq!(cached.address(), address);
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
}
