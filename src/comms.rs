pub mod rpc_comms_client;

pub use rpc_comms_client::RpcCommsClient;

use anyhow::Result;
use solana_sdk::{account::Account, pubkey::Pubkey};

use crate::config::Config;

// TODO: consider renaming this trait to something more descriptive. Fetcher for example.
pub trait CommsClient: Send + Sync {
    fn new(config: &Config) -> Result<Self>
    where
        Self: Sized;

    fn get_account(&self, address: &Pubkey) -> Result<Account>;

    fn get_program_accounts(&self, program_id: &Pubkey) -> Result<Vec<(Pubkey, Account)>>;

    fn get_accounts(&self, addresses: &Vec<Pubkey>) -> Result<Vec<(Pubkey, Account)>>;
}

#[cfg(test)]
pub mod test_util {
    use anyhow::{anyhow, Result};
    use std::collections::HashMap;

    use super::*;

    pub struct MockedCommsClient {
        accounts: HashMap<Pubkey, Account>,
    }

    impl MockedCommsClient {
        pub fn with_accounts(accounts: HashMap<Pubkey, Account>) -> Self {
            Self { accounts }
        }
    }

    impl CommsClient for MockedCommsClient {
        fn new(_config: &Config) -> Result<Self> {
            Ok(Self {
                accounts: HashMap::new(),
            })
        }

        fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
            self.accounts
                .get(pubkey)
                .cloned()
                .ok_or_else(|| anyhow!("Account not found"))
        }

        fn get_program_accounts(&self, program_id: &Pubkey) -> Result<Vec<(Pubkey, Account)>> {
            Ok(self
                .accounts
                .iter()
                .filter(|(&pubkey, _)| pubkey == *program_id)
                .map(|(pubkey, account)| (pubkey.clone(), account.clone()))
                .collect())
        }

        fn get_accounts(&self, pubkeys: &Vec<Pubkey>) -> Result<Vec<(Pubkey, Account)>> {
            let mut accounts = Vec::new();
            for pubkey in pubkeys {
                if let Ok(account) = self.get_account(pubkey) {
                    accounts.push((pubkey.clone(), account));
                }
            }
            Ok(accounts)
        }
    }
}
