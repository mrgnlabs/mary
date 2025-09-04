use std::{collections::HashMap, sync::RwLock};

use marginfi::state::price::{
    OraclePriceFeedAdapter, OracleSetup, PythPushOraclePriceFeed, SwitchboardPullPriceFeed,
};
use solana_sdk::{account::Account, pubkey::Pubkey};

use crate::cache::CacheEntry;
use anyhow::{anyhow, Result};

use log::{trace, warn};

use anchor_lang::prelude::AccountInfo;

use solana_sdk::account_info::IntoAccountInfo;
use switchboard_on_demand::{Discriminator, PullFeedAccountData};

#[derive(Clone)]
pub struct CachedPriceAdapter {
    pub slot: u64,
    _adapter: OraclePriceFeedAdapter,
}

impl CachedPriceAdapter {
    pub fn from(
        slot: u64,
        oracle_type: &OracleSetup,
        address: &Pubkey,
        account: &mut Account,
    ) -> Result<Self> {
        let adapter = match oracle_type {
            OracleSetup::SwitchboardPull => Self::parse_swb_adapter(&account.data)?,
            OracleSetup::PythPushOracle => Self::parse_pyth_adapter(address, account)?,
            _ => return Err(anyhow!("Unsupported oracle type {:?}", oracle_type)),
        };

        Ok(Self {
            slot,
            _adapter: adapter,
        })
    }

    fn parse_swb_adapter(data: &[u8]) -> Result<OraclePriceFeedAdapter> {
        if data.len() < 8 {
            return Err(anyhow!("Invalid Swb oracle account length"));
        }

        if data[..8] != PullFeedAccountData::DISCRIMINATOR {
            return Err(anyhow!(
                "Invalid Swb oracle account discriminator {:?}! Expected {:?}",
                &data[..8],
                PullFeedAccountData::DISCRIMINATOR
            ));
        }

        let feed = bytemuck::try_pod_read_unaligned::<PullFeedAccountData>(
            &data[8..8 + std::mem::size_of::<PullFeedAccountData>()],
        )
        .map_err(|err| anyhow!("Failed to parse the Swb oracle account: {:?}", err))?;

        Ok(OraclePriceFeedAdapter::SwitchboardPull(
            SwitchboardPullPriceFeed {
                feed: Box::new((&feed).into()),
            },
        ))
    }

    fn parse_pyth_adapter(
        &address: &Pubkey,
        account: &mut Account,
    ) -> Result<OraclePriceFeedAdapter> {
        if account.data.len() < 8 {
            return Err(anyhow!("Invalid Pyth oracle account length"));
        }

        let ai: AccountInfo = (&address, account).into_account_info();
        let feed = PythPushOraclePriceFeed::load_unchecked(&ai)?;
        Ok(OraclePriceFeedAdapter::PythPushOracle(feed))
    }
}

#[derive(Clone)]
pub struct CachedOracle {
    pub _address: Pubkey,
    pub _oracle_type: OracleSetup,
    adapter: Option<CachedPriceAdapter>,
}

impl CacheEntry for CachedOracle {}

impl CachedOracle {
    pub fn from(
        address: Pubkey,
        oracle_type: OracleSetup,
        adapter: Option<CachedPriceAdapter>,
    ) -> Self {
        Self {
            _address: address,
            _oracle_type: oracle_type,
            adapter,
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
        address: &Pubkey,
        oracle_type: OracleSetup,
        mut account: Account,
    ) -> Result<()> {
        let adapter: Option<CachedPriceAdapter> =
            match CachedPriceAdapter::from(slot, &oracle_type, address, &mut account) {
                Ok(adapter) => Some(adapter),
                Err(err) => {
                    warn!(
                        "Failed to create the initial OraclePriceAdapter for {:?}: {}",
                        address, err
                    );
                    None
                }
            };

        self.oracles
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to lock the Oracles cache for insert: {}", e))?
            .insert(*address, CachedOracle::from(*address, oracle_type, adapter));

        Ok(())
    }

    pub fn update(&self, slot: u64, address: &Pubkey, account: &mut Account) -> Result<()> {
        let mut oracles = self
            .oracles
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to lock the Oracles cache for update: {}", e))?;

        if let Some(cached_oracle) = oracles.get_mut(address) {
            if slot > cached_oracle.adapter.as_ref().map_or(0, |a| a.slot) {
                match CachedPriceAdapter::from(slot, &cached_oracle._oracle_type, address, account)
                {
                    Ok(adapter) => {
                        cached_oracle.adapter = Some(adapter);
                        trace!("Updated OraclePriceAdapter for {:?}", address);
                    }
                    Err(err) => {
                        warn!(
                            "Failed to create the updated OraclePriceAdapter for {:?}: {}",
                            address, err
                        );
                    }
                }
            }
        }

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
    use anchor_lang::prelude::AnchorSerialize;
    use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;
    use pyth_solana_receiver_sdk::price_update::{PriceFeedMessage, VerificationLevel};
    use switchboard_on_demand::PullFeedAccountData;

    fn dummy_account(oracle_type: OracleSetup) -> Account {
        let mut data = Vec::new();
        if oracle_type == OracleSetup::SwitchboardPull {
            data.extend_from_slice(&PullFeedAccountData::DISCRIMINATOR);
            data.extend_from_slice(&[0u8; std::mem::size_of::<PullFeedAccountData>()]);
        } else {
            data.extend_from_slice(<PriceUpdateV2 as anchor_lang::Discriminator>::DISCRIMINATOR);
            data.extend_from_slice(&[0u8; std::mem::size_of::<PriceUpdateV2>()]);
        }

        Account {
            lamports: 0,
            data,
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
        let account = dummy_account(oracle_type);

        cache.insert(1, &address, oracle_type, account).unwrap();
        let addresses = cache.get_oracle_addresses();
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0], address);
    }

    #[test]
    fn test_update_oracle_price_slot() {
        let cache = OraclesCache::default();
        let address = Pubkey::new_unique();
        let oracle_type = OracleSetup::PythPushOracle;
        let mut account = dummy_account(oracle_type);
        account.owner = pyth_solana_receiver_sdk::id();

        cache
            .insert(1, &address, oracle_type, account.clone())
            .unwrap();
        // Update with a higher slot
        cache.update(2, &address, &mut account).unwrap();

        let oracles = cache.oracles.read().unwrap();
        let cached = oracles.get(&address).unwrap();
        assert_eq!(cached.adapter.as_ref().unwrap().slot, 2);
    }

    #[test]
    fn test_update_oracle_price_slot_lower_no_update() {
        let cache = OraclesCache::default();
        let address = Pubkey::new_unique();
        let oracle_type = OracleSetup::SwitchboardPull;
        let mut account = dummy_account(oracle_type);

        cache
            .insert(5, &address, oracle_type, account.clone())
            .unwrap();
        // Try to update with a lower slot, should not update
        cache.update(3, &address, &mut account).unwrap();

        let oracles = cache.oracles.read().unwrap();
        let cached = oracles.get(&address).unwrap();
        assert_eq!(cached.adapter.as_ref().unwrap().slot, 5);
    }

    #[test]
    fn test_insert_multiple_oracles() {
        let cache = OraclesCache::default();
        let addresses: Vec<_> = (0..5).map(|_| Pubkey::new_unique()).collect();
        let oracle_type = OracleSetup::SwitchboardPull;
        let account = dummy_account(oracle_type);

        for (i, address) in addresses.iter().enumerate() {
            cache
                .insert(i as u64, address, oracle_type.clone(), account.clone())
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
        let mut account = dummy_account(OracleSetup::None);

        // Should not panic or insert anything
        cache.update(10, &address, &mut account).unwrap();
        let addresses = cache.get_oracle_addresses();
        assert!(addresses.is_empty());
    }

    #[test]
    fn test_parse_swb_adapter() {
        // Construct valid data: discriminator + PullFeedAccountData bytes
        let mut data = Vec::new();
        data.extend_from_slice(&PullFeedAccountData::DISCRIMINATOR);

        // Create a PullFeedAccountData with a known value
        data.extend_from_slice(&[0u8; std::mem::size_of::<PullFeedAccountData>()]);
        let adapter = CachedPriceAdapter::parse_swb_adapter(&data);
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_parse_swb_adapter_invalid_length() {
        let data = vec![0u8; 4]; // Too short
        let result = CachedPriceAdapter::parse_swb_adapter(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_swb_adapter_invalid_discriminator() {
        let mut data = vec![1u8; 8]; // Wrong discriminator
        data.extend_from_slice(&vec![
            0u8;
            std::mem::size_of::<
                switchboard_on_demand::PullFeedAccountData,
            >()
        ]);
        let result = CachedPriceAdapter::parse_swb_adapter(&data);
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("Invalid Swb oracle account discriminator"));
    }

    #[test]
    fn test_parse_pyth_adapter_invalid_length() {
        let mut account = dummy_account(OracleSetup::PythPushOracle);
        account.owner = pyth_solana_receiver_sdk::id();
        account.data = vec![0u8; 4]; // Too short
        let result = CachedPriceAdapter::parse_pyth_adapter(&Pubkey::new_unique(), &mut account);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_pyth_adapter_invalid_discriminator() {
        let mut account = dummy_account(OracleSetup::PythPushOracle);
        account.owner = pyth_solana_receiver_sdk::id();
        account.data = vec![1u8; 8]; // Use wrong discriminator
        account.data.extend_from_slice(&vec![0u8; 64]); // Add some bytes for the rest of the account data
        let result = CachedPriceAdapter::parse_pyth_adapter(&Pubkey::new_unique(), &mut account);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_pyth_adapter_valid() {
        // Use correct discriminator but invalid payload (too short for deserialize)
        let mut account = dummy_account(OracleSetup::PythPushOracle);
        account.owner = pyth_solana_receiver_sdk::id();
        let discrim = <PriceUpdateV2 as anchor_lang::Discriminator>::DISCRIMINATOR;
        account.data.extend_from_slice(discrim);

        let price_update = PriceUpdateV2 {
            write_authority: Pubkey::new_unique(),
            verification_level: VerificationLevel::Full,
            price_message: PriceFeedMessage {
                feed_id: [0; 32],
                ema_conf: 0,
                ema_price: 0,
                price: 1234,
                conf: 2,
                exponent: 3,
                prev_publish_time: 899,
                publish_time: 900,
            },
            posted_slot: 0,
        };
        let mut feed = Vec::new();
        price_update.serialize(&mut feed).unwrap();
        account.data.extend_from_slice(&feed);

        let adapter = CachedPriceAdapter::parse_pyth_adapter(&Pubkey::new_unique(), &mut account);
        assert!(adapter.is_ok());
    }
}
