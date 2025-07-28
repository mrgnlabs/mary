use crate::comms::CommsClient;
use crate::config::Config;
use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, pubkey::Pubkey};

// TODO: need better name!
pub struct RpcCommsClient {
    solana_rpc_client: RpcClient,
}

impl CommsClient for RpcCommsClient {
    fn new(config: &Config) -> Result<Self> {
        let solana_rpc_client = RpcClient::new(&config.rpc_url);
        Ok(RpcCommsClient { solana_rpc_client })
    }

    fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        self.solana_rpc_client
            .get_account(pubkey)
            .map_err(|e| anyhow!("Failed to get account {}: {}", pubkey, e))
    }
}
