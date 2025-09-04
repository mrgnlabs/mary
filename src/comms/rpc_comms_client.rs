use crate::comms::{CommsClient};
use crate::config::Config;
use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{account::Account, commitment_config::CommitmentConfig, pubkey::Pubkey};

const ADDRESSES_CHUNK_SIZE: usize = 100;
use solana_client::{rpc_config::RpcSendTransactionConfig};
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::{commitment_config::{ CommitmentLevel}, compute_budget::ComputeBudgetInstruction, instruction::Instruction};

pub struct RpcCommsClient {
    solana_rpc_client: RpcClient,
    cu_limit_ix: Instruction,
    marginfi_account: Pubkey,
    signer: Keypair,
}

impl CommsClient for RpcCommsClient {
    fn new(config: &Config) -> Result<Self> {
        let solana_rpc_client =
            RpcClient::new_with_commitment(&config.rpc_url, CommitmentConfig::confirmed());
        let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(200000);
        let marginfi_account = Pubkey::new_unique(); //config.marginfi_program_id
        let signer = Keypair::new(); //config.keypair
        Ok(RpcCommsClient { solana_rpc_client, cu_limit_ix, marginfi_account, signer })
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

    fn send_ix(&self, ix: Instruction) -> Result<()> {
                let recent_blockhash = self.solana_rpc_client.get_latest_blockhash()?;

        let tx: solana_sdk::transaction::Transaction =
            solana_sdk::transaction::Transaction::new_signed_with_payer(
                &[self.cu_limit_ix.clone(), ix],
                Some(&self.signer.pubkey()),
                &[&self.signer],
                recent_blockhash,
            );

        let _ = self.solana_rpc_client.send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                CommitmentConfig::finalized(),
                RpcSendTransactionConfig {
                    skip_preflight: false,
                    preflight_commitment: Some(CommitmentLevel::Processed),
                    ..Default::default()
                },
            )
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(())
    }
}
