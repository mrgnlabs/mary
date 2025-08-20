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

    fn get_account(&self, pubkey: &Pubkey) -> Result<Account>;
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
    }
}
