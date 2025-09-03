use std::{collections::HashMap, sync::Arc};

use anchor_spl::associated_token;
use log::debug;
use marginfi_type_crate::types::{Bank, LendingAccount};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer, system_program};
use solana_program::{sysvar};
use anyhow::Result;

use crate::{
    cache::{marginfi_accounts::CachedMarginfiAccount, Cache}, liquidation::{utils::maybe_add_bank_mint, CommsClient, LiquidationParams}
};
use anchor_lang::{prelude::AccountMeta, ToAccountMetas};
use anchor_lang::InstructionData;

// Make sure to import or define the LiquidationStrategy trait
use crate::liquidation::LiquidationStrategy;

pub struct BasicLiquidationStrategy<T: CommsClient> {
    comms_client: T,
    cache: Arc<Cache>,
    marginfi_program_id: Pubkey,
    signer: Keypair,
}

impl<T: CommsClient> LiquidationStrategy for BasicLiquidationStrategy<T> {
    pub fn new(comms_client: T, cache: Cache) -> Self {
        let cache = Arc::clone(&cache);

        BasicLiquidationStrategy { comms_client }
    }

    fn prepare(
        &self,
        _account: &CachedMarginfiAccount,
    ) -> anyhow::Result<Option<LiquidationParams>> {
        debug!("Preparing account {:?} for liquidation.", _account);
        Ok(Some(LiquidationParams {}))
    }

    fn liquidate(
        &self,
        liquidation_params: LiquidationParams,
    ) -> anyhow::Result<()> {
        debug!("Liquidating {:?}", liquidation_params);

        self.comms_client.get_account(pubkey)

        Ok(())
    }
}

impl<T: CommsClient> BasicLiquidationStrategy<T> {
        fn start_liquidation(&self, liquidatee_key: Pubkey) -> Result<()>
    {
        let liquidatee = self.cache.marginfi_accounts.get(&liquidatee_key)?;
    let accounts = marginfi::accounts::StartLiquidation {
        marginfi_account: liquidatee_key,
        liquidation_record: liquidatee.liquidation_record,
        liquidation_receiver: self.signer.pubkey(),
        instruction_sysvar: sysvar::instructions::id(),
    }
    .to_account_metas(Some(true));


    let start_ix = Instruction {
        program_id: self.marginfi_program_id,
        accounts,
        data: marginfi::instruction::StartLiquidation {
        }
        .data(),
    };
    
    self.comms_client.send_ix(start_ix)
    }

    fn withdraw(&self, liquidatee_key: Pubkey, bank_key: Pubkey, amount: u64) -> Result<()> {
        let bank = self.cache.banks.get(&bank_key)?;
        let mint_key = bank.mint;
        let mint_account = self.cache.mints.get(&mint_key).unwrap();

        let destination_token_account = associated_token::get_associated_token_address_with_program_id(
                    &self.signer.pubkey(),
                    &mint_key,
                    &mint_account.owner,
                );
    let mut accounts = marginfi::accounts::LendingAccountWithdraw {
        marginfi_account: liquidatee_key,
        destination_token_account,
        liquidity_vault: Pubkey::new_unique(), // TODO: remove
        token_program: mint_account.owner,
        authority: Pubkey::new_unique(), // TODO: remove
        bank_liquidity_vault_authority: Pubkey::new_unique(), // TODO: remove
        bank: bank_key,
        group: Pubkey::new_unique(), // TODO: remove
    }
    .to_account_metas(Some(true));

    maybe_add_bank_mint(&mut accounts, mint_key, &mint_account.owner);

        let lending_account = self.cache.marginfi_accounts.get(&liquidatee_key)?.lending_account;
    
    let banks_map: HashMap<Pubkey, Bank> = self.cache.banks.get_banks_map();
    let observation_accounts = lending_account.load_observation_account_metas(&banks_map, vec![], vec![]); // TODO: optimize

    println!(
        "withdraw: observation_accounts: {:?}",
        observation_accounts
    );

    accounts.extend(
        observation_accounts
            .iter()
            .map(|a| AccountMeta::new_readonly(a.key(), false)),
    );

    let withdraw_ix = Instruction {
        program_id: self.marginfi_program_id,
        accounts,
        data: marginfi::instruction::LendingAccountWithdraw {
            amount,
            withdraw_all: Some(false),
        }
        .data(),
    };
    
    self.comms_client.send_ix(withdraw_ix)
    }

        fn repay(&self, liquidatee_key: Pubkey, bank_key: &Pubkey, amount: u64) -> Result<()> {
        let bank = self.cache.banks.get(bank_key).unwrap();
        let mint_key = bank.mint;
        let mint_account = self.cache.mints.get(&mint_key).unwrap();

        let signer_token_account = associated_token::get_associated_token_address_with_program_id(
                    &self.signer.pubkey(),
                    &mint_key,
                    &mint_account.owner,
                );
    let mut accounts = marginfi::accounts::LendingAccountRepay {
        marginfi_account: liquidatee_key,
        signer_token_account,
        liquidity_vault: Pubkey::new_unique(), // TODO: remove
        token_program: mint_account.owner,
        authority: Pubkey::new_unique(), // TODO: remove
        bank: *bank_key,
        group: Pubkey::new_unique(), // TODO: remove
    }
    .to_account_metas(Some(true));

    maybe_add_bank_mint(&mut accounts, mint_key, &mint_account.owner);

    let observation_accounts = load_observation_account_metas(liquidatee_key); // TODO: export from program

    println!(
        "withdraw: observation_accounts: {:?}",
        observation_accounts
    );

    accounts.extend(
        observation_accounts
            .iter()
            .map(|a| AccountMeta::new_readonly(a.key(), false)),
    );

    let repay_ix = Instruction {
        program_id: self.marginfi_program_id,
        accounts,
        data: marginfi::instruction::LendingAccountRepay {
            amount,
            repay_all: Some(false),
        }
        .data(),
    };
    
    self.comms_client.send_ix(repay_ix)
    }

    fn end_liquidation(&self, liquidatee_key: Pubkey) -> Result<()>
    {
        let liquidatee = self.cache.marginfi_accounts.get(&liquidatee_key)?;
    
    let accounts = marginfi::accounts::EndLiquidation {
        marginfi_account: liquidatee_key,
        liquidation_record: liquidatee.liquidation_record,
        liquidation_receiver: self.signer.pubkey(),
        fee_state: self.cache.global_fee_state_key,
        global_fee_wallet: Pubkey::default(), // TODO: remove
        system_program: system_program::ID,
    }
    .to_account_metas(Some(true));


    let end_ix = Instruction {
        program_id: self.marginfi_program_id,
        accounts,
        data: marginfi::instruction::EndLiquidation {
        }
        .data(),
    };
    
    self.comms_client.send_ix(end_ix)
    }
}
