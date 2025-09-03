use crate::cache::CacheEntry;
use anyhow::{anyhow, Result};
use log::trace;
use solana_sdk::{account::Account, pubkey::Pubkey};
use std::{collections::HashMap, sync::RwLock};

#[derive(Debug, Clone)]
pub struct CachedMint {
    pub _address: Pubkey,
    pub owner: Pubkey,
}

impl CacheEntry for CachedMint {}

#[derive(Default)]
pub struct MintsCache {
    mints: RwLock<HashMap<Pubkey, CachedMint>>,
}

impl MintsCache {
    pub fn update(&self, address: Pubkey, mint: &Account) -> Result<()> {
        let upd_cached_mint = CachedMint {
            _address: address,
            owner: mint.owner,
        };

        trace!("Updating the Mint in cache: {:?}", upd_cached_mint);

        self.mints
            .write()
            .map_err(|e| anyhow!("Failed to lock the Mints cache for update: {}", e))?
            .insert(address, upd_cached_mint);

        Ok(())
    }

    pub fn get(&self, address: &Pubkey) -> Result<CachedMint> {
        self
            .mints
            .read()
            .map_err(|e| anyhow!("Failed to lock the Mints cache for read: {}", e))?
            .get(address)
            .cloned().ok_or(anyhow!("Failed to get mint from the Mints cache: {}", address))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_inserts_new_mint() {
        let cache = MintsCache::default();
        let address = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let account = Account {
            lamports: 0,
            data: vec![],
            owner,
            executable: false,
            rent_epoch: 0,
        };

        assert!(cache.update(address, &account).is_ok());

        let mints = cache.mints.read().unwrap();
        let cached = mints.get(&address).unwrap();
        assert_eq!(cached._address, address);
        assert_eq!(cached.owner, owner);
    }

    #[test]
    fn test_update_overwrites_existing_mint() {
        let cache = MintsCache::default();
        let address = Pubkey::new_unique();
        let owner1 = Pubkey::new_unique();
        let owner2 = Pubkey::new_unique();

        let account1 = Account {
            lamports: 0,
            data: vec![],
            owner: owner1,
            executable: false,
            rent_epoch: 0,
        };
        let account2 = Account {
            lamports: 0,
            data: vec![],
            owner: owner2,
            executable: false,
            rent_epoch: 0,
        };

        cache.update(address, &account1).unwrap();
        cache.update(address, &account2).unwrap();

        let mints = cache.mints.read().unwrap();
        let cached = mints.get(&address).unwrap();
        assert_eq!(cached.owner, owner2);
    }

    #[test]
    fn test_get_returns_none_for_missing_mint() {
        let cache = MintsCache::default();
        let address = Pubkey::new_unique();
        let result = cache.get(&address);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_returns_some_for_existing_mint() {
        let cache = MintsCache::default();
        let address = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let account = Account {
            lamports: 0,
            data: vec![],
            owner,
            executable: false,
            rent_epoch: 0,
        };

        cache.update(address, &account).unwrap();
        let result = cache.get(&address);
        assert!(result.is_ok());
        let cached = result.unwrap();
        assert_eq!(cached._address, address);
        assert_eq!(cached.owner, owner);
    }
}
