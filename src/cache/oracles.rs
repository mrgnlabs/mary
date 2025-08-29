use std::{collections::HashMap, sync::RwLock};

use marginfi::state::price::OracleSetup;
use solana_sdk::{account::Account, pubkey::Pubkey};

use crate::cache::CacheEntry;
use anyhow::Result;
use log::trace;

#[derive(Debug, Clone)]
pub struct CachedOraclePrice {
    pub slot: u64,
    pub _price: f64,
}

impl CachedOraclePrice {
    pub fn from(slot: u64, _account: &Account) -> Self {
        // TODO: recover from the account
        let price = 0.0;
        Self {
            slot,
            _price: price,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedOracle {
    pub _address: Pubkey,
    pub _oracle_type: OracleSetup,
    pub price: CachedOraclePrice,
}

impl CacheEntry for CachedOracle {}

impl CachedOracle {
    pub fn from(slot: u64, address: Pubkey, oracle_type: OracleSetup, account: Account) -> Self {
        Self {
            _address: address,
            _oracle_type: oracle_type,
            price: CachedOraclePrice::from(slot, &account),
        }
    }
}

#[derive(Default)]
pub struct OraclesCache {
    oracles: RwLock<HashMap<Pubkey, CachedOracle>>,
}

impl OraclesCache {
    pub fn insert(
        &self,
        slot: u64,
        address: Pubkey,
        oracle_type: OracleSetup,
        account: Account,
    ) -> Result<()> {
        self.oracles
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to lock the Oracles cache for insert: {}", e))?
            .insert(
                address,
                CachedOracle::from(slot, address, oracle_type, account),
            );

        Ok(())
    }

    pub fn update(&self, slot: u64, address: &Pubkey, account: &Account) -> Result<()> {
        self.oracles
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to lock the Oracles cache for update: {}", e))?
            .entry(*address)
            .and_modify(|cached_oracle| {
                if slot > cached_oracle.price.slot {
                    trace!("Updating the Oracle in cache: {:?}", cached_oracle);
                    cached_oracle.price = CachedOraclePrice::from(slot, account);
                }
            });

        Ok(())
    }

    pub fn get(&self, address: &Pubkey) -> Result<Option<CachedOracle>> {
        Ok(self
            .oracles
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to lock the Oracles cache for read: {}", e))?
            .get(address)
            .cloned())
    }

    pub fn get_oracle_addresses(&self) -> Vec<Pubkey> {
        self.oracles
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to lock the Oracles cache for read: {}", e))
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_account() -> Account {
        Account {
            lamports: 0,
            data: vec![],
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
        }
    }

    #[test]
    fn test_insert_and_get_oracle_addresses() {
        let cache = OraclesCache::default();
        let address = Pubkey::new_unique();
        let oracle_type = OracleSetup::PythPushOracle;
        let account = dummy_account();

        cache.insert(1, address, oracle_type, account).unwrap();
        let addresses = cache.get_oracle_addresses();
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0], address);
    }

    #[test]
    fn test_update_oracle_price_slot() {
        let cache = OraclesCache::default();
        let address = Pubkey::new_unique();
        let oracle_type = OracleSetup::PythPushOracle;
        let account = dummy_account();

        cache
            .insert(1, address, oracle_type, account.clone())
            .unwrap();
        // Update with a higher slot
        cache.update(2, &address, &account).unwrap();

        let oracles = cache.oracles.read().unwrap();
        let cached = oracles.get(&address).unwrap();
        assert_eq!(cached.price.slot, 2);
    }

    #[test]
    fn test_update_oracle_price_slot_lower_no_update() {
        let cache = OraclesCache::default();
        let address = Pubkey::new_unique();
        let oracle_type = OracleSetup::SwitchboardPull;
        let account = dummy_account();

        cache
            .insert(5, address, oracle_type, account.clone())
            .unwrap();
        // Try to update with a lower slot, should not update
        cache.update(3, &address, &account).unwrap();

        let oracles = cache.oracles.read().unwrap();
        let cached = oracles.get(&address).unwrap();
        assert_eq!(cached.price.slot, 5);
    }

    #[test]
    fn test_insert_multiple_oracles() {
        let cache = OraclesCache::default();
        let addresses: Vec<_> = (0..5).map(|_| Pubkey::new_unique()).collect();
        let oracle_type = OracleSetup::SwitchboardPull;
        let account = dummy_account();

        for (i, address) in addresses.iter().enumerate() {
            cache
                .insert(i as u64, *address, oracle_type.clone(), account.clone())
                .unwrap();
        }

        let stored_addresses = cache.get_oracle_addresses();
        assert_eq!(stored_addresses.len(), 5);
        for address in addresses {
            assert!(stored_addresses.contains(&address));
        }
    }

    #[test]
    fn test_update_nonexistent_oracle_does_nothing() {
        let cache = OraclesCache::default();
        let address = Pubkey::new_unique();
        let account = dummy_account();

        // Should not panic or insert anything
        cache.update(10, &address, &account).unwrap();
        let addresses = cache.get_oracle_addresses();
        assert!(addresses.is_empty());
    }
}
