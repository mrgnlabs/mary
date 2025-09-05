use std::{collections::HashMap, sync::RwLock};

use anyhow::{anyhow, Result};
use fixed::types::I80F48;
use log::{trace, warn};
use marginfi::state::marginfi_account::{Balance, MarginfiAccount};
use solana_sdk::pubkey::Pubkey;

use crate::cache::CacheEntry;

#[derive(Clone)]
pub struct CachedMarginfiAccount {
    slot: u64,
    address: Pubkey,
    _marginfi_account: MarginfiAccount,
    _positions: Vec<Balance>,
}

const INVALID_HEALTH: i64 = i64::MIN;

impl std::fmt::Debug for CachedMarginfiAccount {
    // TODO: add more relevant fields
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedMarginfiAccount")
            .field("slot", &self.slot)
            .field("address", &self.address)
            .finish()
    }
}

impl CacheEntry for CachedMarginfiAccount {}

impl CachedMarginfiAccount {
    pub fn from(slot: u64, address: Pubkey, marginfi_account: MarginfiAccount) -> Self {
        let positions = marginfi_account
            .lending_account
            .balances
            .iter()
            .filter(|balance| balance.active != 0)
            .cloned()
            .collect();

        Self {
            slot,
            address,
            _marginfi_account: marginfi_account,
            _positions: positions,
        }
    }

    #[inline]
    pub fn asset_value_maint(&self) -> I80F48 {
        self._marginfi_account.health_cache.asset_value_maint.into()
    }

    #[inline]
    pub fn liability_value_maint(&self) -> I80F48 {
        self._marginfi_account
            .health_cache
            .liability_value_maint
            .into()
    }

    #[inline]
    pub fn health(&self) -> Option<i64> {
        (self.asset_value_maint() - self.liability_value_maint())
            .checked_div(self.asset_value_maint())
            .map(|v| v.to_num::<i64>())
    }

    pub fn _positions(&self) -> &Vec<Balance> {
        &self._positions
    }
}

#[derive(Default)]
pub struct MarginfiAccountsCache {
    accounts: RwLock<HashMap<Pubkey, CachedMarginfiAccount>>,
    account_to_health: RwLock<HashMap<Pubkey, i64>>,
}

impl MarginfiAccountsCache {
    pub fn update(&self, slot: u64, address: Pubkey, account: MarginfiAccount) -> Result<()> {
        let upd_cached_account = CachedMarginfiAccount::from(slot, address, account);
        let upd_cached_account_health = upd_cached_account.health();

        let mut accounts = self.accounts.write().map_err(|e| {
            anyhow!(
                "Failed to lock the Marginfi accounts cache for update! {}",
                e
            )
        })?;
        let mut health = self.account_to_health.write().map_err(|e| {
            anyhow!(
                "Failed to lock the Marginfi account health cache for update! {}",
                e
            )
        })?;

        if accounts
            .get(&address)
            .map_or(true, |existing| existing.slot < upd_cached_account.slot)
        {
            trace!(
                "Updating the Marginfi Account in cache: {:?}",
                upd_cached_account
            );
            accounts.insert(address, upd_cached_account);

            match upd_cached_account_health {
                Some(upd_health) => {
                    health.insert(address, upd_health);
                }
                None => {
                    warn!(
                        "Failed to compute health for account {}, invalidating it",
                        address
                    );
                    health.insert(address, INVALID_HEALTH);
                }
            }
        }

        Ok(())
    }

    pub fn get_account(&self, address: &Pubkey) -> Result<CachedMarginfiAccount> {
        self.accounts
            .read()
            .map_err(|e| {
                anyhow!(
                    "Failed to lock the Marginfi accounts cache for getting an account: {}",
                    e
                )
            })?
            .get(address)
            .cloned()
            .ok_or_else(|| anyhow!("Account {} not found in cache", address))
    }

    pub fn get_accounts_with_health(&self) -> Result<HashMap<Pubkey, i64>> {
        Ok(self
            .account_to_health
            .read()
            .map_err(|e| {
                anyhow!(
                    "Failed to lock the Marginfi account health cache for cloning: {}",
                    e
                )
            })?
            .clone())
    }
}

#[cfg(test)]
pub mod test_util {
    use fixed::types::I80F48;
    use marginfi::state::{
        health_cache::HealthCache,
        marginfi_account::{Balance, LendingAccount, MarginfiAccount},
        marginfi_group::WrappedI80F48,
    };
    use solana_sdk::pubkey::Pubkey;

    pub fn create_default_balance() -> Balance {
        Balance {
            active: 0,
            bank_pk: Pubkey::default(),
            bank_asset_tag: 0,
            _pad0: [0; 6],
            asset_shares: WrappedI80F48::default(),
            liability_shares: WrappedI80F48::default(),
            emissions_outstanding: WrappedI80F48::default(),
            last_update: 0,
            _padding: [0_u64],
        }
    }

    pub fn create_balance(bank: Pubkey, asset: i64, liability: i64) -> Balance {
        Balance {
            bank_pk: bank,
            asset_shares: WrappedI80F48::from(I80F48::from_num(asset)),
            liability_shares: WrappedI80F48::from(I80F48::from_num(liability)),
            active: 1,
            bank_asset_tag: 0,
            _pad0: [0; 6],
            emissions_outstanding: WrappedI80F48::default(),
            last_update: 0,
            _padding: [0_u64],
            // Add other required fields here with appropriate dummy/test values
        }
    }

    pub fn create_marginfi_account(group: Pubkey, balances: Vec<Balance>) -> MarginfiAccount {
        let mut balances_array: [Balance; 16] = std::array::from_fn(|_| create_default_balance());

        for (i, val) in balances.into_iter().enumerate().take(16) {
            balances_array[i] = val;
        }

        MarginfiAccount {
            group,
            lending_account: LendingAccount {
                balances: balances_array,
                _padding: [0; 8],
            },
            account_flags: 0,
            migrated_from: Pubkey::default(),
            migrated_to: Pubkey::default(),
            health_cache: HealthCache {
                // Fill in the fields with appropriate dummy/test values
                ..unsafe { std::mem::zeroed() }
            },
            _padding0: [0; 13],
            authority: Pubkey::default(),
            emissions_destination_account: Pubkey::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::{create_balance, create_marginfi_account};
    use super::*;
    use fixed::types::I80F48;
    use marginfi::state::marginfi_group::WrappedI80F48;
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn test_cached_marginfi_account_from() {
        let slot = 42;
        let address = Pubkey::new_unique();
        let group = Pubkey::new_unique();
        let bank1 = Pubkey::new_unique();
        let bank2 = Pubkey::new_unique();

        let balances = vec![
            create_balance(bank1, 100, 50),
            create_balance(bank2, 200, 75),
        ];
        let marginfi_account = create_marginfi_account(group, balances.clone());

        let cached = CachedMarginfiAccount::from(slot, address, marginfi_account);

        assert_eq!(cached.slot, slot);
        assert_eq!(cached.address, address);
        assert_eq!(cached._positions().len(), 2);
        assert_eq!(cached._positions()[0].bank_pk, bank1);
        assert_eq!(cached._positions()[1].bank_pk, bank2);
        assert_eq!(
            cached._positions()[0].asset_shares,
            WrappedI80F48::from(I80F48::from_num(100))
        );
        assert_eq!(
            cached._positions()[0].liability_shares,
            WrappedI80F48::from(I80F48::from_num(50))
        );
        assert_eq!(
            cached._positions()[1].asset_shares,
            WrappedI80F48::from(I80F48::from_num(200))
        );
        assert_eq!(
            cached._positions()[1].liability_shares,
            WrappedI80F48::from(I80F48::from_num(75))
        );
    }

    #[test]
    fn test_marginfi_accounts_cache_update_and_retrieve() {
        let cache = MarginfiAccountsCache::default();
        let slot = 100;
        let address = Pubkey::new_unique();
        let group = Pubkey::new_unique();
        let bank = Pubkey::new_unique();
        let balances = vec![create_balance(bank, 10, 5)];
        let marginfi_account = create_marginfi_account(group, balances);

        cache
            .update(slot, address, marginfi_account)
            .expect("update should succeed");

        let cached = cache
            .get_account(&address)
            .expect("account should be cached");
        assert_eq!(cached.slot, slot);
        assert_eq!(cached.address, address);
        assert_eq!(cached._positions().len(), 1);
        assert_eq!(cached._positions()[0].bank_pk, bank);

        let health_map = cache.get_accounts_with_health().unwrap();
        assert_eq!(health_map.get(&address), Some(&INVALID_HEALTH));
    }

    #[test]
    fn test_update_overwrites_existing_account() {
        let cache = MarginfiAccountsCache::default();
        let address = Pubkey::new_unique();
        let group1 = Pubkey::new_unique();
        let group2 = Pubkey::new_unique();
        let bank1 = Pubkey::new_unique();
        let bank2 = Pubkey::new_unique();

        let marginfi_account1 = create_marginfi_account(group1, vec![create_balance(bank1, 1, 2)]);
        let marginfi_account2 = create_marginfi_account(group2, vec![create_balance(bank2, 3, 4)]);

        cache
            .update(1, address, marginfi_account1)
            .expect("first update");
        cache
            .update(2, address, marginfi_account2)
            .expect("second update");

        let cached = cache.get_account(&address).unwrap();
        assert_eq!(cached.slot, 2);
        assert_eq!(cached._positions()[0].bank_pk, bank2);

        let health_map = cache.get_accounts_with_health().unwrap();
        assert_eq!(health_map.get(&address), Some(&INVALID_HEALTH));
    }

    #[test]
    fn test_update_with_older_slot_does_not_overwrite() {
        let cache = MarginfiAccountsCache::default();
        let address = Pubkey::new_unique();
        let group_new = Pubkey::new_unique();
        let group_old = Pubkey::new_unique();
        let bank_new = Pubkey::new_unique();
        let bank_old = Pubkey::new_unique();

        let marginfi_account_new =
            create_marginfi_account(group_new, vec![create_balance(bank_new, 10, 20)]);
        let marginfi_account_old =
            create_marginfi_account(group_old, vec![create_balance(bank_old, 30, 40)]);

        // Insert with higher slot first
        cache
            .update(10, address, marginfi_account_new)
            .expect("first update with new slot");

        // Try to update with lower slot
        cache
            .update(5, address, marginfi_account_old)
            .expect("second update with old slot");

        let cached = cache.get_account(&address).unwrap();
        // Should still have the new slot and data
        assert_eq!(cached.slot, 10);
        assert_eq!(cached._positions()[0].bank_pk, bank_new);
        assert_eq!(
            cached._positions()[0].asset_shares,
            WrappedI80F48::from(I80F48::from_num(10))
        );
        assert_eq!(
            cached._positions()[0].liability_shares,
            WrappedI80F48::from(I80F48::from_num(20))
        );
    }

    #[test]
    fn test_get_account_returns_error_for_missing_account() {
        let cache = MarginfiAccountsCache::default();
        let address = Pubkey::new_unique();
        let result = cache.get_account(&address);
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("not found in cache"));
    }

    #[test]
    fn test_get_accounts_with_health_empty() {
        let cache = MarginfiAccountsCache::default();
        let health_map = cache.get_accounts_with_health().unwrap();
        assert!(health_map.is_empty());
    }

    #[test]
    fn test_multiple_accounts_in_cache() {
        let cache = MarginfiAccountsCache::default();
        let slot1 = 1;
        let slot2 = 2;
        let address1 = Pubkey::new_unique();
        let address2 = Pubkey::new_unique();
        let group1 = Pubkey::new_unique();
        let group2 = Pubkey::new_unique();
        let bank1 = Pubkey::new_unique();
        let bank2 = Pubkey::new_unique();

        let marginfi_account1 =
            create_marginfi_account(group1, vec![create_balance(bank1, 11, 22)]);
        let marginfi_account2 =
            create_marginfi_account(group2, vec![create_balance(bank2, 33, 44)]);

        cache.update(slot1, address1, marginfi_account1).unwrap();
        cache.update(slot2, address2, marginfi_account2).unwrap();

        let cached1 = cache.get_account(&address1).unwrap();
        let cached2 = cache.get_account(&address2).unwrap();

        assert_eq!(cached1.slot, slot1);
        assert_eq!(cached2.slot, slot2);
        assert_eq!(cached1._positions()[0].bank_pk, bank1);
        assert_eq!(cached2._positions()[0].bank_pk, bank2);

        let health_map = cache.get_accounts_with_health().unwrap();
        assert_eq!(health_map.get(&address1), Some(&INVALID_HEALTH));
        assert_eq!(health_map.get(&address2), Some(&INVALID_HEALTH));
    }

    #[test]
    fn test_asset_value_maint_and_liability_value_maint() {
        let slot = 1;
        let address = Pubkey::new_unique();
        let group = Pubkey::new_unique();
        let bank = Pubkey::new_unique();

        let mut marginfi_account =
            create_marginfi_account(group, vec![create_balance(bank, 100, 50)]);
        // Set health_cache values
        marginfi_account.health_cache.asset_value_maint = I80F48::from_num(500).into();
        marginfi_account.health_cache.liability_value_maint = I80F48::from_num(200).into();

        let cached = CachedMarginfiAccount::from(slot, address, marginfi_account);

        assert_eq!(cached.asset_value_maint(), I80F48::from_num(500));
        assert_eq!(cached.liability_value_maint(), I80F48::from_num(200));
    }

    #[test]
    fn test_health_returns_some_when_asset_value_maint_nonzero() {
        let slot = 1;
        let address = Pubkey::new_unique();
        let group = Pubkey::new_unique();
        let bank = Pubkey::new_unique();

        let mut marginfi_account =
            create_marginfi_account(group, vec![create_balance(bank, 100, 50)]);
        marginfi_account.health_cache.asset_value_maint = I80F48::from_num(1000).into();
        marginfi_account.health_cache.liability_value_maint = I80F48::from_num(500).into();

        let cached = CachedMarginfiAccount::from(slot, address, marginfi_account);

        // health = (1000 - 500) / 1000 = 0.5 -> to_num::<u64>() = 0
        assert_eq!(cached.health(), Some(0));
    }

    #[test]
    fn test_health_returns_none_when_asset_value_maint_zero() {
        let slot = 1;
        let address = Pubkey::new_unique();
        let group = Pubkey::new_unique();
        let bank = Pubkey::new_unique();

        let mut marginfi_account =
            create_marginfi_account(group, vec![create_balance(bank, 100, 50)]);
        marginfi_account.health_cache.asset_value_maint = I80F48::from_num(0).into();
        marginfi_account.health_cache.liability_value_maint = I80F48::from_num(500).into();

        let cached = CachedMarginfiAccount::from(slot, address, marginfi_account);

        assert_eq!(cached.health(), None);
    }

    #[test]
    fn test_health_negative_liability() {
        let slot = 1;
        let address = Pubkey::new_unique();
        let group = Pubkey::new_unique();
        let bank = Pubkey::new_unique();

        let mut marginfi_account =
            create_marginfi_account(group, vec![create_balance(bank, 100, 50)]);
        marginfi_account.health_cache.asset_value_maint = I80F48::from_num(1000).into();
        marginfi_account.health_cache.liability_value_maint = I80F48::from_num(1500).into();

        let cached = CachedMarginfiAccount::from(slot, address, marginfi_account);

        // health = (1000 - 1500) / 1000 = -0.5 -> to_num::<i64>() = -1
        assert_eq!(cached.health(), Some(-1));
    }
}
