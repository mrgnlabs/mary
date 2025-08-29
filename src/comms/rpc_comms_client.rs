use crate::comms::CommsClient;
use crate::config::Config;
use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, commitment_config::CommitmentConfig, pubkey::Pubkey};

const ADDRESSES_CHUNK_SIZE: usize = 100;

pub struct RpcCommsClient {
    solana_rpc_client: RpcClient,
}

impl CommsClient for RpcCommsClient {
    fn new(config: &Config) -> Result<Self> {
        let solana_rpc_client =
            RpcClient::new_with_commitment(&config.rpc_url, CommitmentConfig::confirmed());
        Ok(RpcCommsClient { solana_rpc_client })
    }

    fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        self.solana_rpc_client
            .get_account(pubkey)
            .map_err(|e| anyhow!("Failed to get account {}: {}", pubkey, e))
    }

    fn get_program_accounts(&self, program_id: &Pubkey) -> Result<Vec<(Pubkey, Account)>> {
        self.solana_rpc_client
            .get_program_accounts(program_id)
            .map_err(|e| anyhow!("Failed to get accounts for program{}: {}", program_id, e))
    }

    fn get_accounts(&self, addresses: &[Pubkey]) -> Result<Vec<(Pubkey, Account)>> {
        let mut tuples: Vec<(Pubkey, Account)> = Vec::new();

        for chunk in addresses.chunks(ADDRESSES_CHUNK_SIZE) {
            let accounts = self.solana_rpc_client.get_multiple_accounts(chunk)?;
            for (address, account_opt) in chunk.iter().zip(accounts.iter()) {
                if let Some(account) = account_opt {
                    tuples.push((*address, account.clone()));
                }
            }
        }

        Ok(tuples)
    }
}
