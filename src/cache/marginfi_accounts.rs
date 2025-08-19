use std::{collections::HashMap, sync::RwLock};

use anyhow::anyhow;
use fixed::types::I80F48;
use marginfi::state::marginfi_account::{Balance, MarginfiAccount};
use solana_sdk::pubkey::Pubkey;

pub struct CachedPosition {
    pub bank: Pubkey,
    // TODO: make sure that we really need to use the I80F48 type here. It depends on what type is used for calling the protocol API
    pub asset_shares: I80F48,
    pub liability_shares: I80F48,
}

impl CachedPosition {
    pub fn from(balance: &Balance) -> Self {
        Self {
            bank: balance.bank_pk,
            asset_shares: balance.asset_shares.into(),
            liability_shares: balance.liability_shares.into(),
        }
    }
}

pub struct CachedMarginfiAccount {
    pub slot: u64,
    pub address: Pubkey,
    pub group: Pubkey,
    pub health: u64,
    pub positions: Vec<CachedPosition>,
}

impl CachedMarginfiAccount {
    pub fn from(slot: u64, address: Pubkey, marginfi_account: &MarginfiAccount) -> Self {
        let positions = marginfi_account
            .lending_account
            .balances
            .iter()
            .filter(|balance| balance.active != 0)
            .map(CachedPosition::from)
            .collect();

        Self {
            slot,
            address,
            group: marginfi_account.group,
            health: 0, //TODO: either recover from the MarginfiAccount.HealthCache or replace with meaningful HealthCache properties
            positions,
        }
    }
}

#[derive(Default)]
pub struct MarginfiAccountsCache {
    accounts: RwLock<HashMap<Pubkey, CachedMarginfiAccount>>,
}

impl MarginfiAccountsCache {
    pub fn update(
        &self,
        slot: u64,
        address: Pubkey,
        account: &MarginfiAccount,
    ) -> anyhow::Result<()> {
        let cached_account = CachedMarginfiAccount::from(slot, address, account);
        self.accounts
            .write()
            .map_err(|e| {
                anyhow!(
                    "Failed to lock the marginfi accounts cache for update! {}",
                    e
                )
            })?
            .insert(address, cached_account);
        Ok(())
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;
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
            health_cache: HealthCache {
                // Fill in the fields with appropriate dummy/test values
                ..unsafe { std::mem::zeroed() }
            },
            _padding0: [0; 17],
            authority: Pubkey::default(),
            emissions_destination_account: Pubkey::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::{create_balance, create_marginfi_account};
    use super::*;
    use solana_sdk::pubkey::Pubkey;

    #[test]
    fn test_cached_position_from_balance() {
        let bank = Pubkey::new_unique();
        let asset = 12345;
        let liability = 6789;
        let balance = create_balance(bank, asset, liability);

        let cached = CachedPosition::from(&balance);

        assert_eq!(cached.bank, bank);
        assert_eq!(cached.asset_shares, I80F48::from_num(asset));
        assert_eq!(cached.liability_shares, I80F48::from_num(liability));
    }

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

        let cached = CachedMarginfiAccount::from(slot, address, &marginfi_account);

        assert_eq!(cached.slot, slot);
        assert_eq!(cached.address, address);
        assert_eq!(cached.group, group);
        assert_eq!(cached.positions.len(), 2);
        assert_eq!(cached.positions[0].bank, bank1);
        assert_eq!(cached.positions[1].bank, bank2);
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
            .update(slot, address, &marginfi_account)
            .expect("update should succeed");

        let accounts = cache.accounts.read().unwrap();
        let cached = accounts.get(&address).expect("account should be cached");
        assert_eq!(cached.slot, slot);
        assert_eq!(cached.address, address);
        assert_eq!(cached.group, group);
        assert_eq!(cached.positions.len(), 1);
        assert_eq!(cached.positions[0].bank, bank);
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
            .update(1, address, &marginfi_account1)
            .expect("first update");
        cache
            .update(2, address, &marginfi_account2)
            .expect("second update");

        let accounts = cache.accounts.read().unwrap();
        let cached = accounts.get(&address).unwrap();
        assert_eq!(cached.slot, 2);
        assert_eq!(cached.group, group2);
        assert_eq!(cached.positions[0].bank, bank2);
    }
}
