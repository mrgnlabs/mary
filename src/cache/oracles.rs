use std::{collections::HashMap, sync::RwLock};

use marginfi::state::price::OracleSetup;
use solana_sdk::{account::Account, pubkey::Pubkey};

use crate::cache::CacheEntry;
use anyhow::{anyhow, Result};

use log::{trace, warn};

use anchor_lang::prelude::AnchorDeserialize;

use bytemuck;
use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;
use switchboard_on_demand::{Discriminator, PullFeedAccountData};

type CachedPriceType = i128;

#[derive(Debug, Clone)]
pub struct CachedOraclePrice {
    pub slot: u64,
    pub price: CachedPriceType,
}

const ZERO_PRICE: CachedOraclePrice = CachedOraclePrice { slot: 0, price: 0 };
const PYTH_DISCRIMINATOR: &[u8] = <PriceUpdateV2 as anchor_lang::Discriminator>::DISCRIMINATOR;

impl CachedOraclePrice {
    pub fn from(slot: u64, oracle_type: &OracleSetup, account: &Account) -> Result<Self> {
        let price = match oracle_type {
            OracleSetup::SwitchboardPull => Self::parse_swb_price(&account.data)?,
            OracleSetup::PythPushOracle => Self::parse_pyth_price(&account.data)?,
            _ => return Err(anyhow!("Unsupported oracle type {:?}", oracle_type)),
        };

        Ok(Self { slot, price })
    }

    // Inspired by https://github.com/mrgnlabs/marginfi-v2/blob/8ec81a6b302c2c65d58e563cf5d1e45ce2ab0a6a/programs/marginfi/src/state/price.rs#L577
    fn parse_swb_price(data: &[u8]) -> Result<CachedPriceType> {
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
        .map_err(|err| {
            anyhow!(
                "Failed to read price from the Swb oracle account: {:?}",
                err
            )
        })?;

        Ok(feed.result.value)
    }

    // Inspired by https://github.com/mrgnlabs/marginfi-v2/blob/8ec81a6b302c2c65d58e563cf5d1e45ce2ab0a6a/programs/marginfi/src/state/price.rs#L594
    fn parse_pyth_price(data: &[u8]) -> Result<CachedPriceType> {
        if data.len() < 8 {
            return Err(anyhow!("Invalid Pyth oracle account length"));
        }

        if &data[..8] != PYTH_DISCRIMINATOR {
            return Err(anyhow!(
                "Invalid Pyth oracle account discriminator {:?}! Expected {:?}",
                &data[..8],
                PYTH_DISCRIMINATOR
            ));
        }

        let feed: PriceUpdateV2 = PriceUpdateV2::deserialize(&mut &data[8..])?;

        Ok(feed.price_message.price as CachedPriceType)
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
    pub fn from(address: Pubkey, oracle_type: OracleSetup, price: CachedOraclePrice) -> Self {
        Self {
            _address: address,
            _oracle_type: oracle_type,
            price,
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
        let price = match CachedOraclePrice::from(slot, &oracle_type, &account) {
            Ok(price) => price,
            Err(err) => {
                warn!(
                    "Failed to parse initial CachedOraclePrice for {:?}: {}",
                    address, err
                );
                ZERO_PRICE
            }
        };

        self.oracles
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to lock the Oracles cache for insert: {}", e))?
            .insert(address, CachedOracle::from(address, oracle_type, price));

        Ok(())
    }

    pub fn update(&self, slot: u64, address: &Pubkey, account: &Account) -> Result<()> {
        let mut oracles = self
            .oracles
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to lock the Oracles cache for update: {}", e))?;

        if let Some(cached_oracle) = oracles.get_mut(address) {
            if slot > cached_oracle.price.slot {
                match CachedOraclePrice::from(slot, &cached_oracle._oracle_type, &account) {
                    Ok(price) => {
                        cached_oracle.price = price;
                        trace!(
                            "Updated CachedOraclePrice for {:?} to {:?}",
                            address,
                            cached_oracle.price
                        );
                    }
                    Err(err) => {
                        warn!(
                            "Failed to parse updated CachedOraclePrice for {:?}: {}",
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
        let account = dummy_account(oracle_type);

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
        let account = dummy_account(oracle_type);

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
        let account = dummy_account(oracle_type);

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
        let account = dummy_account(OracleSetup::None);

        // Should not panic or insert anything
        cache.update(10, &address, &account).unwrap();
        let addresses = cache.get_oracle_addresses();
        assert!(addresses.is_empty());
    }

    #[test]
    fn test_parse_swb_price() {
        // Construct valid data: discriminator + PullFeedAccountData bytes
        let mut data = Vec::new();
        data.extend_from_slice(&PullFeedAccountData::DISCRIMINATOR);

        // Create a PullFeedAccountData with a known value
        data.extend_from_slice(&[0u8; std::mem::size_of::<PullFeedAccountData>()]);
        let price = CachedOraclePrice::parse_swb_price(&data).unwrap();
        assert_eq!(price, 0);
    }

    #[test]
    fn test_parse_swb_price_invalid_length() {
        let data = vec![0u8; 4]; // Too short
        let result = CachedOraclePrice::parse_swb_price(&data);
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("Invalid Swb oracle account length"));
    }

    #[test]
    fn test_parse_swb_price_invalid_discriminator() {
        let mut data = vec![1u8; 8]; // Wrong discriminator
        data.extend_from_slice(&vec![
            0u8;
            std::mem::size_of::<
                switchboard_on_demand::PullFeedAccountData,
            >()
        ]);
        let result = CachedOraclePrice::parse_swb_price(&data);
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("Invalid Swb oracle account discriminator"));
    }

    #[test]
    fn test_parse_pyth_price_invalid_length() {
        let data = vec![0u8; 4]; // Too short
        let result = CachedOraclePrice::parse_pyth_price(&data);
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("Invalid Pyth oracle account length"));
    }

    #[test]
    fn test_parse_pyth_price_invalid_discriminator() {
        // Use wrong discriminator
        let mut data = vec![1u8; 8];
        // Add some bytes for the rest of the account data
        data.extend_from_slice(&vec![0u8; 64]);
        let result = CachedOraclePrice::parse_pyth_price(&data);
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("Invalid Pyth oracle account discriminator"));
    }

    #[test]
    fn test_parse_pyth_price_valid() {
        // Use correct discriminator but invalid payload (too short for deserialize)
        let mut data = Vec::new();
        let discrim = <PriceUpdateV2 as anchor_lang::Discriminator>::DISCRIMINATOR;
        data.extend_from_slice(discrim);

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
        data.extend_from_slice(&feed);

        let price = CachedOraclePrice::parse_pyth_price(&data).unwrap();
        assert_eq!(price_update.price_message.price, price as i64);
    }
}
