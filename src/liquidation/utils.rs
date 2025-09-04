use anchor_lang::prelude::AccountMeta;
use marginfi::{bank_authority_seed, state::bank::BankVaultType};
use solana_sdk::pubkey::Pubkey;
use anchor_spl::token_2022;

pub fn find_bank_liquidity_vault_authority(bank_pk: &Pubkey, program_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        bank_authority_seed!(BankVaultType::Liquidity, bank_pk),
        program_id,
    )
    .0
}

pub fn maybe_add_bank_mint(accounts: &mut Vec<AccountMeta>, mint: Pubkey, token_program: &Pubkey) {
    if token_program == &token_2022::ID {
        println!("!!!Adding mint account to accounts!!!");
        accounts.push(AccountMeta::new_readonly(mint, false));
    }
}