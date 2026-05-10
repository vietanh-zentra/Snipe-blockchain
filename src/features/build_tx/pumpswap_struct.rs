use crate::*;
use borsh::BorshDeserialize;
#[allow(deprecated)]
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_instruction,
};
use spl_associated_token_account::{
    get_associated_token_address, get_associated_token_address_with_program_id,
    instruction::{create_associated_token_account, create_associated_token_account_idempotent},
};
use spl_token::instruction::sync_native;

#[derive(Debug, Clone, BorshDeserialize, Copy)]
pub struct PumpSwapStruct {
    pub pool: Pubkey,
    pub global_config: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub pool_base_token_account: Pubkey,
    pub pool_quote_token_account: Pubkey,
    pub protocol_fee_recipient: Pubkey,
    pub protocol_fee_recipient_token_account: Pubkey,
    pub base_token_program: Pubkey,
    pub quote_token_program: Pubkey,
    pub system_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub event_authority: Pubkey,
    pub program: Pubkey,
    pub fee_config: Pubkey,
    pub fee_program: Pubkey,
    pub pool_v2_pda: Pubkey,
}

impl PumpSwapStruct {
    #[allow(deprecated)]
    pub fn from_migrate(
        _migrate_accounts: &MigrateInstructionAccounts,
        create_pool_accounts: &CreatePoolInstructionAccounts,
        create_pool_event_data: &CreatePoolEventData,
    ) -> Self {
        let protocol_fee_recipient = if create_pool_event_data.is_mayhem_mode {
            MAYHEM_PROTOCOL_FEE_RECIPIENT
        } else {
            PUMPSWAP_FEE_1
        };

        let protocol_fee_recipient_token_account = get_associated_token_address_with_program_id(
            &protocol_fee_recipient,
            &create_pool_accounts.quote_mint,
            &create_pool_accounts.quote_token_program,
        );

        let (pool_v2_pda, _) = Pubkey::find_program_address(
            &[
                PUMPSWAP_POOL_V2_SEED,
                &create_pool_accounts.base_mint.as_ref(),
            ],
            &PUMPSWAP_PROGRAM_ID,
        );

        Self {
            pool: create_pool_accounts.pool,
            global_config: create_pool_accounts.global_config,
            base_mint: create_pool_accounts.base_mint,
            quote_mint: create_pool_accounts.quote_mint,
            pool_base_token_account: create_pool_accounts.pool_base_token_account,
            pool_quote_token_account: create_pool_accounts.pool_quote_token_account,
            protocol_fee_recipient: protocol_fee_recipient,
            protocol_fee_recipient_token_account: protocol_fee_recipient_token_account,
            base_token_program: create_pool_accounts.base_token_program,
            quote_token_program: create_pool_accounts.quote_token_program,
            system_program: create_pool_accounts.system_program,
            associated_token_program: create_pool_accounts.associated_token_program,
            event_authority: create_pool_accounts.event_authority,
            program: PUMPSWAP_PROGRAM_ID,
            fee_config: PUMPSWAP_FEE_CONFIG,
            fee_program: PUMPSWAP_FEE_PROGRAM,
            pool_v2_pda: pool_v2_pda,
        }
    }

    pub fn get_create_ata_ix(&self, signer_pubkey: &Pubkey) -> Vec<Instruction> {
        let create_base_ata = create_associated_token_account(
            signer_pubkey,
            signer_pubkey,
            &self.base_mint,
            &self.base_token_program,
        );
        let create_quote_ata = create_associated_token_account(
            signer_pubkey,
            signer_pubkey,
            &self.quote_mint,
            &self.quote_token_program,
        );

        vec![create_base_ata, create_quote_ata]
    }

    pub fn get_create_ata_idempotent_ix(&self, signer_pubkey: &Pubkey) -> Vec<Instruction> {
        let create_base_ata = create_associated_token_account_idempotent(
            signer_pubkey,
            signer_pubkey,
            &self.base_mint,
            &self.base_token_program,
        );
        let create_quote_ata = create_associated_token_account_idempotent(
            signer_pubkey,
            signer_pubkey,
            &self.quote_mint,
            &self.quote_token_program,
        );

        vec![create_base_ata, create_quote_ata]
    }

    #[allow(deprecated)]
    pub fn get_wsol_ix(
        &self,
        signer_pubkey: &Pubkey,
        buy_amount: f64,
        slippage: f64,
    ) -> Vec<Instruction> {
        let slippage_calculated_buy_amount = buy_amount * 1e9 * (100.0 + slippage) / 100.0;
        let turncated_slippage_calculated_buy_amount =
            slippage_calculated_buy_amount.trunc() as u64;
        let wsol_ata = get_associated_token_address(signer_pubkey, &WSOL);
        let transfer_ix = system_instruction::transfer(
            signer_pubkey,
            &wsol_ata,
            turncated_slippage_calculated_buy_amount,
        );
        let wrap_ix = sync_native(&spl_token::ID, &wsol_ata).unwrap();

        vec![transfer_ix, wrap_ix]
    }

    pub fn close_wsol_ata(&self, signer_pubkey: &Pubkey) -> Instruction {
        let wsol_ata = get_associated_token_address(signer_pubkey, &WSOL);
        let ix = spl_token::instruction::close_account(
            &self.quote_token_program,
            &wsol_ata,
            signer_pubkey,
            signer_pubkey,
            &[signer_pubkey],
        )
        .unwrap();
        ix
    }

    pub fn get_buy_ix(
        &mut self,
        signer_pubkey: &Pubkey,
        updated_coin_creator: Pubkey,
        is_cashback_enabled: bool,
        token_price: f64,
        sol_amount: f64,
        slippage: f64,
    ) -> Instruction {
        //get custom accounts
        let user_base_token_account = get_associated_token_address_with_program_id(
            signer_pubkey,
            &self.base_mint,
            &self.base_token_program,
        );

        let user_quote_token_account = get_associated_token_address_with_program_id(
            signer_pubkey,
            &self.quote_mint,
            &self.quote_token_program,
        );

        let user_volume_accumulator = get_pumpswap_user_volume_accumulator(*signer_pubkey);

        let (coin_creator_vault_authority, _bump) = Pubkey::find_program_address(
            &[PUMPSWAP_CREATOR_VAULT_SEED, updated_coin_creator.as_ref()],
            &PUMPSWAP_PROGRAM_ID,
        );

        let coin_creator_vault_authority_ata = get_associated_token_address_with_program_id(
            &coin_creator_vault_authority,
            &self.quote_mint,
            &self.quote_token_program,
        );

        //build instruction data
        let mut data = Vec::new();

        // Official PumpSwap: buy(baseOut, maxQuoteIn)
        // base_out = 50% of expected tokens (aggressive buffer for fast price movement)
        // max_quote_in = sol_amount + slippage buffer (ceiling for SOL spent)
        // For sniping: competing bots push price up fast. 50% ensures tx success.
        let expected_tokens: f64 = (sol_amount / token_price) * 10f64.powi(6);
        let base_out: u64 = (expected_tokens * 0.50).max(1.0) as u64;
        let max_quote_in: u64 = (sol_amount * 10f64.powi(9) * (100.0 + slippage) / 100.0) as u64;

        data.extend_from_slice(&BUY_DISCRIMINATOR);
        data.extend_from_slice(&base_out.to_le_bytes());
        data.extend_from_slice(&max_quote_in.to_le_bytes());

        // Buyback fee recipient ATA
        let buyback_fee_recipient_token_account = get_associated_token_address_with_program_id(
            &PUMPSWAP_BUYBACK_FEE_RECIPIENT,
            &self.quote_mint,
            &self.quote_token_program,
        );

        let accounts = if !is_cashback_enabled {
            vec![
                AccountMeta::new(self.pool, false),                    // #1 - Pool
                AccountMeta::new(*signer_pubkey, true), // #2 - User (Signer, Writable, Fee Payer)
                AccountMeta::new_readonly(self.global_config, false), // #3 - Global Config
                AccountMeta::new_readonly(self.base_mint, false), // #4 - Base Mint
                AccountMeta::new_readonly(self.quote_mint, false), // #5 - Quote Mint
                AccountMeta::new(user_base_token_account, false), // #6 - User Base Token Account
                AccountMeta::new(user_quote_token_account, false), // #7 - User Quote Token Account
                AccountMeta::new(self.pool_base_token_account, false), // #8 - Pool Base Token Account
                AccountMeta::new(self.pool_quote_token_account, false), // #9 - Pool Quote Token Account
                AccountMeta::new_readonly(self.protocol_fee_recipient, false), // #10 - Protocol Fee Recipient
                AccountMeta::new(self.protocol_fee_recipient_token_account, false), // #11 - Protocol Fee Recipient Token Account
                AccountMeta::new_readonly(self.base_token_program, false), // #12 - Base Token Program
                AccountMeta::new_readonly(self.quote_token_program, false), // #13 - Quote Token Program
                AccountMeta::new_readonly(self.system_program, false),      // #14 - System Program
                AccountMeta::new_readonly(self.associated_token_program, false), // #15 - Associated Token Program
                AccountMeta::new_readonly(self.event_authority, false), // #16 - Event Authority
                AccountMeta::new_readonly(self.program, false), // #17 - Program (Pump.fun AMM)
                AccountMeta::new(coin_creator_vault_authority_ata, false), // #18 - Coin Creator Vault ATA
                AccountMeta::new_readonly(coin_creator_vault_authority, false), // #19 - Coin Creator Vault Authority
                AccountMeta::new(PUMPSWAP_GLOBAL_VOLUME_ACCUMULATOR, false), // #20 - Global Volume Accumulator
                AccountMeta::new(user_volume_accumulator, false), // #21 - User Volume Accumulator
                AccountMeta::new_readonly(self.fee_config, false), // #22 - Fee Config
                AccountMeta::new_readonly(self.fee_program, false), // #23 - Fee Program
                // remaining_accounts: pool_v2_pda FIRST, then buyback (per official docs)
                AccountMeta::new_readonly(self.pool_v2_pda, false), // #24 - Pool V2 PDA
                AccountMeta::new_readonly(PUMPSWAP_BUYBACK_FEE_RECIPIENT, false), // #25 - Buyback Fee Recipient
                AccountMeta::new(buyback_fee_recipient_token_account, false), // #26 - Buyback Fee Recipient Token Account
            ]
        } else {
            let user_volume_accumulator_wsol_ata = get_associated_token_address_with_program_id(
                &user_volume_accumulator,
                &WSOL,
                &self.quote_token_program,
            );
            vec![
                AccountMeta::new(self.pool, false),                    // #1 - Pool
                AccountMeta::new(*signer_pubkey, true), // #2 - User (Signer, Writable, Fee Payer)
                AccountMeta::new_readonly(self.global_config, false), // #3 - Global Config
                AccountMeta::new_readonly(self.base_mint, false), // #4 - Base Mint
                AccountMeta::new_readonly(self.quote_mint, false), // #5 - Quote Mint
                AccountMeta::new(user_base_token_account, false), // #6 - User Base Token Account
                AccountMeta::new(user_quote_token_account, false), // #7 - User Quote Token Account
                AccountMeta::new(self.pool_base_token_account, false), // #8 - Pool Base Token Account
                AccountMeta::new(self.pool_quote_token_account, false), // #9 - Pool Quote Token Account
                AccountMeta::new_readonly(self.protocol_fee_recipient, false), // #10 - Protocol Fee Recipient
                AccountMeta::new(self.protocol_fee_recipient_token_account, false), // #11 - Protocol Fee Recipient Token Account
                AccountMeta::new_readonly(self.base_token_program, false), // #12 - Base Token Program
                AccountMeta::new_readonly(self.quote_token_program, false), // #13 - Quote Token Program
                AccountMeta::new_readonly(self.system_program, false),      // #14 - System Program
                AccountMeta::new_readonly(self.associated_token_program, false), // #15 - Associated Token Program
                AccountMeta::new_readonly(self.event_authority, false), // #16 - Event Authority
                AccountMeta::new_readonly(self.program, false), // #17 - Program (Pump.fun AMM)
                AccountMeta::new(coin_creator_vault_authority_ata, false), // #18 - Coin Creator Vault ATA
                AccountMeta::new_readonly(coin_creator_vault_authority, false), // #19 - Coin Creator Vault Authority
                AccountMeta::new(PUMPSWAP_GLOBAL_VOLUME_ACCUMULATOR, false), // #20 - Global Volume Accumulator
                AccountMeta::new(user_volume_accumulator, false), // #21 - User Volume Accumulator
                AccountMeta::new_readonly(self.fee_config, false), // #22 - Fee Config
                AccountMeta::new_readonly(self.fee_program, false), // #23 - Fee Program
                AccountMeta::new(user_volume_accumulator_wsol_ata, false), // #24 - User volume accumulator wsol ata
                // remaining_accounts: pool_v2_pda FIRST, then buyback (per official docs)
                AccountMeta::new_readonly(self.pool_v2_pda, false),        // #25 - Pool V2 PDA
                AccountMeta::new_readonly(PUMPSWAP_BUYBACK_FEE_RECIPIENT, false), // #26 - Buyback Fee Recipient
                AccountMeta::new(buyback_fee_recipient_token_account, false), // #27 - Buyback Fee Recipient Token Account
            ]
        };

        Instruction {
            program_id: PUMPSWAP_PROGRAM_ID,
            accounts,
            data,
        }
    }

    pub fn get_sell_ix(
        &mut self,
        signer_pubkey: &Pubkey,
        sell_amount: u64,
        updated_coin_creator: Pubkey,
        is_cashback_enabled: bool,
        min_sol_output: u64,
    ) -> Instruction {
        //get custom accounts
        let user_base_token_account = get_associated_token_address_with_program_id(
            signer_pubkey,
            &self.base_mint,
            &self.base_token_program,
        );

        let user_quote_token_account = get_associated_token_address_with_program_id(
            signer_pubkey,
            &self.quote_mint,
            &self.quote_token_program,
        );

        let (coin_creator_vault_authority, _bump) = Pubkey::find_program_address(
            &[PUMPSWAP_CREATOR_VAULT_SEED, updated_coin_creator.as_ref()],
            &PUMPSWAP_PROGRAM_ID,
        );

        let coin_creator_vault_authority_ata = get_associated_token_address_with_program_id(
            &coin_creator_vault_authority,
            &self.quote_mint,
            &self.quote_token_program,
        );

        //build instruction data
        let mut data = Vec::new();

        let min_sol_out: u64 = min_sol_output;

        data.extend_from_slice(&SELL_DISCRIMINATOR);
        data.extend_from_slice(&sell_amount.to_le_bytes());
        data.extend_from_slice(&min_sol_out.to_le_bytes());

        // Buyback fee recipient ATA
        let buyback_fee_recipient_token_account = get_associated_token_address_with_program_id(
            &PUMPSWAP_BUYBACK_FEE_RECIPIENT,
            &self.quote_mint,
            &self.quote_token_program,
        );

        let accounts = if !is_cashback_enabled {
            vec![
                AccountMeta::new(self.pool, false),                    // #1 - Pool
                AccountMeta::new(*signer_pubkey, true), // #2 - User (Signer, Writable, Fee Payer)
                AccountMeta::new_readonly(self.global_config, false), // #3 - Global Config
                AccountMeta::new_readonly(self.base_mint, false), // #4 - Base Mint
                AccountMeta::new_readonly(self.quote_mint, false), // #5 - Quote Mint
                AccountMeta::new(user_base_token_account, false), // #6 - User Base Token Account
                AccountMeta::new(user_quote_token_account, false), // #7 - User Quote Token Account
                AccountMeta::new(self.pool_base_token_account, false), // #8 - Pool Base Token Account
                AccountMeta::new(self.pool_quote_token_account, false), // #9 - Pool Quote Token Account
                AccountMeta::new_readonly(self.protocol_fee_recipient, false), // #10 - Protocol Fee Recipient
                AccountMeta::new(self.protocol_fee_recipient_token_account, false), // #11 - Protocol Fee Recipient Token Account
                AccountMeta::new_readonly(self.base_token_program, false), // #12 - Base Token Program
                AccountMeta::new_readonly(self.quote_token_program, false), // #13 - Quote Token Program
                AccountMeta::new_readonly(self.system_program, false),      // #14 - System Program
                AccountMeta::new_readonly(self.associated_token_program, false), // #15 - Associated Token Program
                AccountMeta::new_readonly(self.event_authority, false), // #16 - Event Authority
                AccountMeta::new_readonly(self.program, false), // #17 - Program (Pump.fun AMM)
                AccountMeta::new(coin_creator_vault_authority_ata, false), // #18 - Coin Creator Vault ATA
                AccountMeta::new_readonly(coin_creator_vault_authority, false), // #19 - Coin Creator Vault Authority
                AccountMeta::new_readonly(self.fee_config, false),              // #20 - Fee Config
                AccountMeta::new_readonly(self.fee_program, false),             // #21 - Fee Program
                // remaining_accounts: pool_v2_pda FIRST, then buyback (per official docs)
                AccountMeta::new_readonly(self.pool_v2_pda, false),             // #22 - Pool V2 PDA
                AccountMeta::new_readonly(PUMPSWAP_BUYBACK_FEE_RECIPIENT, false), // #23 - Buyback Fee Recipient
                AccountMeta::new(buyback_fee_recipient_token_account, false), // #24 - Buyback Fee Recipient Token Account
            ]
        } else {
            let user_volume_accumulator = get_pumpswap_user_volume_accumulator(*signer_pubkey);
            let user_volume_accumulator_wsol_ata = get_associated_token_address_with_program_id(
                &user_volume_accumulator,
                &WSOL,
                &self.quote_token_program,
            );
            vec![
                AccountMeta::new(self.pool, false),                    // #1 - Pool
                AccountMeta::new(*signer_pubkey, true), // #2 - User (Signer, Writable, Fee Payer)
                AccountMeta::new_readonly(self.global_config, false), // #3 - Global Config
                AccountMeta::new_readonly(self.base_mint, false), // #4 - Base Mint
                AccountMeta::new_readonly(self.quote_mint, false), // #5 - Quote Mint
                AccountMeta::new(user_base_token_account, false), // #6 - User Base Token Account
                AccountMeta::new(user_quote_token_account, false), // #7 - User Quote Token Account
                AccountMeta::new(self.pool_base_token_account, false), // #8 - Pool Base Token Account
                AccountMeta::new(self.pool_quote_token_account, false), // #9 - Pool Quote Token Account
                AccountMeta::new_readonly(self.protocol_fee_recipient, false), // #10 - Protocol Fee Recipient
                AccountMeta::new(self.protocol_fee_recipient_token_account, false), // #11 - Protocol Fee Recipient Token Account
                AccountMeta::new_readonly(self.base_token_program, false), // #12 - Base Token Program
                AccountMeta::new_readonly(self.quote_token_program, false), // #13 - Quote Token Program
                AccountMeta::new_readonly(self.system_program, false),      // #14 - System Program
                AccountMeta::new_readonly(self.associated_token_program, false), // #15 - Associated Token Program
                AccountMeta::new_readonly(self.event_authority, false), // #16 - Event Authority
                AccountMeta::new_readonly(self.program, false), // #17 - Program (Pump.fun AMM)
                AccountMeta::new(coin_creator_vault_authority_ata, false), // #18 - Coin Creator Vault ATA
                AccountMeta::new_readonly(coin_creator_vault_authority, false), // #19 - Coin Creator Vault Authority
                AccountMeta::new_readonly(self.fee_config, false),              // #20 - Fee Config
                AccountMeta::new_readonly(self.fee_program, false),             // #21 - Fee Program
                AccountMeta::new(user_volume_accumulator_wsol_ata, false), // #22 - User volume accumulator wsol ata
                AccountMeta::new(user_volume_accumulator, false), // #23 - User volume accumulator
                // remaining_accounts: pool_v2_pda FIRST, then buyback (per official docs)
                AccountMeta::new_readonly(self.pool_v2_pda, false), // #24 - Pool V2 PDA
                AccountMeta::new_readonly(PUMPSWAP_BUYBACK_FEE_RECIPIENT, false), // #25 - Buyback Fee Recipient
                AccountMeta::new(buyback_fee_recipient_token_account, false), // #26 - Buyback Fee Recipient Token Account
            ]
        };

        Instruction {
            program_id: PUMPSWAP_PROGRAM_ID,
            accounts,
            data,
        }
    }
}
