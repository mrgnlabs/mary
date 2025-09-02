use std::sync::RwLock;

use anyhow::{anyhow, Result};
use solana_sdk::address_lookup_table::AddressLookupTableAccount;

#[derive(Default)]
// TODO: the LUTs cache is effectively read-only after population. Come up with better way to share it lock free
pub struct LutsCache {
    luts: RwLock<Vec<AddressLookupTableAccount>>,
}

impl LutsCache {
    pub fn populate(&self, luts: Vec<AddressLookupTableAccount>) -> Result<()> {
        let mut write_guard = self
            .luts
            .write()
            .map_err(|e| anyhow!("Failed to lock the Mints cache for update: {}", e))?;
        *write_guard = luts;
        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<AddressLookupTableAccount>> {
        let read_guard = self
            .luts
            .read()
            .map_err(|e| anyhow!("Failed to lock the LUTs cache for reading: {}", e))?;
        Ok(read_guard.clone())
    }
}

#[cfg(test)]
mod tests {
    use solana_sdk::pubkey::Pubkey;

    use super::*;

    fn dummy_lut(key: Pubkey) -> AddressLookupTableAccount {
        AddressLookupTableAccount {
            key,
            addresses: vec![Pubkey::new_unique(), Pubkey::new_unique()],
        }
    }

    #[test]
    fn test_populate_success() {
        let cache = LutsCache::default();
        let lut_1 = dummy_lut(Pubkey::new_unique());
        let lut_2 = dummy_lut(Pubkey::new_unique());
        let luts = vec![lut_1.clone(), lut_2.clone()];
        let result = cache.populate(luts.clone());
        assert!(result.is_ok());
        let read_guard = cache.luts.read().unwrap();
        assert_eq!(read_guard.len(), 2);
        assert_eq!(read_guard[0].key, lut_1.key);
        assert_eq!(read_guard[1].key, lut_2.key);
    }

    #[test]
    fn test_populate_overwrites_existing() {
        let cache = LutsCache::default();
        let luts1 = vec![dummy_lut(Pubkey::new_unique())];
        let luts2 = vec![
            dummy_lut(Pubkey::new_unique()),
            dummy_lut(Pubkey::new_unique()),
        ];
        cache.populate(luts1).unwrap();
        cache.populate(luts2.clone()).unwrap();
        let read_guard = cache.luts.read().unwrap();
        assert_eq!(*read_guard, luts2);
    }

    #[test]
    fn test_populate_empty_vec() {
        let cache = LutsCache::default();
        let luts = vec![];
        let result = cache.populate(luts.clone());
        assert!(result.is_ok());
        let read_guard = cache.luts.read().unwrap();
        assert_eq!(read_guard.len(), 0);
    }
}
