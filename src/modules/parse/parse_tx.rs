use crate::*;
use borsh::BorshDeserialize;
use solana_sdk::{bs58, pubkey::Pubkey};
use yellowstone_grpc_proto::{
    geyser::{SubscribeUpdate, subscribe_update::UpdateOneof},
    prelude::{CompiledInstruction, InnerInstruction, Message},
};

pub fn extract_transaction_data(
    update: &SubscribeUpdate,
) -> Option<(
    Vec<Pubkey>,
    Vec<CompiledInstruction>,
    Vec<InnerInstruction>,
    String,
    Vec<Pubkey>,
)> {
    let transaction_update = match &update.update_oneof {
        Some(UpdateOneof::Transaction(tx_update)) => tx_update,
        _ => return None,
    };

    let tx_info = transaction_update.transaction.as_ref()?;
    let transaction = tx_info.transaction.as_ref()?;
    let meta = tx_info.meta.as_ref()?;
    let tx_msg = transaction.message.as_ref()?;

    let (_, signers) = get_signers(tx_msg.clone());

    let mut account_keys: Vec<Pubkey> = tx_msg
        .account_keys
        .iter()
        .filter_map(|key_bytes| Pubkey::try_from(key_bytes.as_slice()).ok())
        .collect();

    account_keys.extend(
        meta.loaded_writable_addresses
            .iter()
            .filter_map(|key_bytes| Pubkey::try_from(key_bytes.as_slice()).ok()),
    );

    account_keys.extend(
        meta.loaded_readonly_addresses
            .iter()
            .filter_map(|key_bytes| Pubkey::try_from(key_bytes.as_slice()).ok()),
    );

    let ixs: Vec<CompiledInstruction> = tx_msg.instructions.clone();
    let inner_ixs: Vec<InnerInstruction> = meta
        .inner_instructions
        .iter()
        .flat_map(|ix| ix.instructions.clone())
        .collect();

    let signature = tx_info.signature.clone();
    let tx_id = bs58::encode(signature).into_string();

    Some((account_keys, ixs, inner_ixs, tx_id, signers))
}

pub fn get_signers(tx_msg: Message) -> (usize, Vec<Pubkey>) {
    let signer_count = tx_msg
        .header
        .map(|header| header.num_required_signatures as usize)
        .unwrap_or(0);

    let pubkeys: Vec<Pubkey> = tx_msg
        .account_keys
        .iter()
        .filter_map(|key_bytes| Pubkey::try_from(key_bytes.as_slice()).ok())
        .collect();

    let signer_pubkeys = &pubkeys[..signer_count.min(pubkeys.len())];
    (signer_count, signer_pubkeys.to_vec())
}
///////Pumpswap trade info extraction
///
///
///
///
pub fn migrate_info(
    infos: Vec<InstructionRawData>,
    account_keys: Vec<Pubkey>,
    tx_id: &str,
) -> (
    Vec<MigrateInstructionAccounts>,
    Vec<CreatePoolInstructionAccounts>,
    Vec<CreatePoolEventData>,
    Vec<CreatePoolInstructionData>,
) {
    let mut migrate_accounts: Vec<MigrateInstructionAccounts> = vec![];
    let mut create_pool_accounts: Vec<CreatePoolInstructionAccounts> = vec![];
    let mut create_pool_events: Vec<CreatePoolEventData> = vec![];
    let mut create_pool_instruction_data: Vec<CreatePoolInstructionData> = vec![];

    infos.iter().for_each(|info| {
        if info.data.starts_with(&MIGRATE_DISCRIMINATOR) {
            let migrate_account = MigrateInstructionAccounts {
                global: account_keys[info.accounts[0] as usize],
                withdraw_authority: account_keys[info.accounts[1] as usize],
                mint: account_keys[info.accounts[2] as usize],
                bonding_curve: account_keys[info.accounts[3] as usize],
                associated_bonding_curve: account_keys[info.accounts[4] as usize],
                user: account_keys[info.accounts[5] as usize],
                system_program: account_keys[info.accounts[6] as usize],
                token_program: account_keys[info.accounts[7] as usize],
                pump_amm_program: account_keys[info.accounts[8] as usize],
                pool: account_keys[info.accounts[9] as usize],
                pool_authority: account_keys[info.accounts[10] as usize],
                pool_authority_mint_account: account_keys[info.accounts[11] as usize],
                pool_authority_wsol_account: account_keys[info.accounts[12] as usize],
                amm_global_config: account_keys[info.accounts[13] as usize],
                wsol_mint: account_keys[info.accounts[14] as usize],
                lp_mint: account_keys[info.accounts[15] as usize],
                user_pool_token_account: account_keys[info.accounts[16] as usize],
                pool_base_token_account: account_keys[info.accounts[17] as usize],
                pool_quote_token_account: account_keys[info.accounts[18] as usize],
                token_2022_program: account_keys[info.accounts[19] as usize],
                associated_token_program: account_keys[info.accounts[20] as usize],
                pump_amm_event_authority: account_keys[info.accounts[21] as usize],
                event_authority: account_keys[info.accounts[22] as usize],
                pump_fun_program: account_keys[info.accounts[23] as usize],
            };

            migrate_accounts.push(migrate_account);
        } else if info.data.starts_with(&CREATE_POOL_DISCRIMINATOR) {
            let create_pool_account = CreatePoolInstructionAccounts {
                pool: account_keys[info.accounts[0] as usize],
                global_config: account_keys[info.accounts[1] as usize],
                creator: account_keys[info.accounts[2] as usize],
                base_mint: account_keys[info.accounts[3] as usize],
                quote_mint: account_keys[info.accounts[4] as usize],
                lp_mint: account_keys[info.accounts[5] as usize],
                user_base_token_account: account_keys[info.accounts[6] as usize],
                user_quote_token_account: account_keys[info.accounts[7] as usize],
                user_pool_token_account: account_keys[info.accounts[8] as usize],
                pool_base_token_account: account_keys[info.accounts[9] as usize],
                pool_quote_token_account: account_keys[info.accounts[10] as usize],
                system_program: account_keys[info.accounts[11] as usize],
                token_2022_program: account_keys[info.accounts[12] as usize],
                base_token_program: account_keys[info.accounts[13] as usize],
                quote_token_program: account_keys[info.accounts[14] as usize],
                associated_token_program: account_keys[info.accounts[15] as usize],
                event_authority: account_keys[info.accounts[16] as usize],
                pump_amm_program: account_keys[info.accounts[17] as usize],
            };

            create_pool_accounts.push(create_pool_account);

            let mut data = &info.data[8..];
            match CreatePoolInstructionData::deserialize_from_slice(&mut data) {
                Ok(instruction_data) => create_pool_instruction_data.push(instruction_data),
                Err(e) => {
                    error!("[PARSE_FAIL] CreatePoolInstructionData deserialize failed\t*Tx: {}\t*Error: {:?}", tx_id, e);
                }
            }
        } else if info.data.starts_with(
            &[
                PUMP_FUN_EVENT_LOG_DISCRIMINATOR,
                CREATE_POOL_EVENT_DISCRIMINATOR,
            ]
            .concat(),
        ) {
            let mut data = &info.data[16..];
            match CreatePoolEventData::deserialize(&mut data) {
                Ok(event) => create_pool_events.push(event),
                Err(e) => {
                    error!("[PARSE_FAIL] CreatePoolEventData deserialize failed\t*Tx: {}\t*Error: {:?}", tx_id, e);
                }
            }
        }
    });

    (
        migrate_accounts,
        create_pool_accounts,
        create_pool_events,
        create_pool_instruction_data,
    )
}

pub fn get_pumpswap_trade_info(
    infos: Vec<InstructionRawData>,
    account_keys: Vec<Pubkey>,
    tx_id: &str,
) -> (
    Vec<PumpswapBuyEvent>,
    Vec<PumpswapSellEvent>,
    Vec<PumpswapBuyInstructionAccounts>,
    Vec<PumpswapSellInstructionAccounts>,
) {
    let mut buy_events: Vec<PumpswapBuyEvent> = vec![];
    let mut sell_events: Vec<PumpswapSellEvent> = vec![];
    let mut buy_accounts: Vec<PumpswapBuyInstructionAccounts> = vec![];
    let mut sell_accounts: Vec<PumpswapSellInstructionAccounts> = vec![];
    infos.iter().for_each(|info| {
        if info.data.starts_with(&BUY_DISCRIMINATOR)
            || info.data.starts_with(&BUY_EXACT_QUOTE_IN_DISCRIMINATOR)
        {
            let buy_account = PumpswapBuyInstructionAccounts {
                pool: account_keys[info.accounts[0] as usize],
                user: account_keys[info.accounts[1] as usize],
                global_config: account_keys[info.accounts[2] as usize],
                base_mint: account_keys[info.accounts[3] as usize],
                quote_mint: account_keys[info.accounts[4] as usize],
                user_base_token_account: account_keys[info.accounts[5] as usize],
                user_quote_token_account: account_keys[info.accounts[6] as usize],
                pool_base_token_account: account_keys[info.accounts[7] as usize],
                pool_quote_token_account: account_keys[info.accounts[8] as usize],
                protocol_fee_recipient: account_keys[info.accounts[9] as usize],
                protocol_fee_recipient_token_account: account_keys[info.accounts[10] as usize],
                base_token_program: account_keys[info.accounts[11] as usize],
                quote_token_program: account_keys[info.accounts[12] as usize],
                system_program: account_keys[info.accounts[13] as usize],
                associated_token_program: account_keys[info.accounts[14] as usize],
                event_authority: account_keys[info.accounts[15] as usize],
                program: account_keys[info.accounts[16] as usize],
                coin_creator_vault_ata: account_keys[info.accounts[17] as usize],
                coin_creator_vault_authority: account_keys[info.accounts[18] as usize],
                global_volume_accumulator: account_keys[info.accounts[19] as usize],
                user_volume_accumulator: account_keys[info.accounts[20] as usize],
                fee_config: account_keys[info.accounts[21] as usize],
                fee_program: account_keys[info.accounts[22] as usize],
            };

            buy_accounts.push(buy_account);
        } else if info.data.starts_with(&SELL_DISCRIMINATOR) {
            let sell_account = PumpswapSellInstructionAccounts {
                pool: account_keys[info.accounts[0] as usize],
                user: account_keys[info.accounts[1] as usize],
                global_config: account_keys[info.accounts[2] as usize],
                base_mint: account_keys[info.accounts[3] as usize],
                quote_mint: account_keys[info.accounts[4] as usize],
                user_base_token_account: account_keys[info.accounts[5] as usize],
                user_quote_token_account: account_keys[info.accounts[6] as usize],
                pool_base_token_account: account_keys[info.accounts[7] as usize],
                pool_quote_token_account: account_keys[info.accounts[8] as usize],
                protocol_fee_recipient: account_keys[info.accounts[9] as usize],
                protocol_fee_recipient_token_account: account_keys[info.accounts[10] as usize],
                base_token_program: account_keys[info.accounts[11] as usize],
                quote_token_program: account_keys[info.accounts[12] as usize],
                system_program: account_keys[info.accounts[13] as usize],
                associated_token_program: account_keys[info.accounts[14] as usize],
                event_authority: account_keys[info.accounts[15] as usize],
                program: account_keys[info.accounts[16] as usize],
                coin_creator_vault_ata: account_keys[info.accounts[17] as usize],
                coin_creator_vault_authority: account_keys[info.accounts[18] as usize],
                fee_config: PUMPSWAP_FEE_CONFIG,
                fee_program: PUMPSWAP_FEE_PROGRAM,
            };

            sell_accounts.push(sell_account);
        } else if info
            .data
            .starts_with(&[EVENT_AUTH_ACC_DISC, BUY_EVENT_DISC].concat())
        {
            let mut data = &info.data[16..];
            match PumpswapBuyEvent::deserialize(&mut data) {
                Ok(event) => buy_events.push(event),
                Err(e) => {
                    error!(
                        "[PARSE_FAIL] PumpswapBuyEvent deserialize failed\t*Tx: {}\t*Error: {:?}",
                        tx_id, e
                    );
                }
            }
        } else if info
            .data
            .starts_with(&[EVENT_AUTH_ACC_DISC, SELL_EVENT_DISC].concat())
        {
            let mut data = &info.data[16..];
            match PumpswapSellEvent::deserialize(&mut data) {
                Ok(event) => sell_events.push(event),
                Err(e) => {
                    error!(
                        "[PARSE_FAIL] PumpswapSellEvent deserialize failed\t*Tx: {}\t*Error: {:?}",
                        tx_id, e
                    );
                }
            }
        }
    });

    (buy_events, sell_events, buy_accounts, sell_accounts)
}

pub fn filter_by_program_id(
    ixs: Vec<CompiledInstruction>,
    inner_ixs: Vec<InnerInstruction>,
    program_id: Pubkey,
    account_keys: Vec<Pubkey>,
) -> Result<Vec<InstructionRawData>, Box<dyn std::error::Error>> {
    let program_id_index = match account_keys.iter().position(|&pos| pos == program_id) {
        Some(index) => index,
        None => {
            // println!("Program not found");
            return Ok(vec![]);
        }
    };

    let filtered_ixs = ixs
        .into_iter()
        .filter(|ix| ix.program_id_index == program_id_index as u32)
        .map(|ix| InstructionRawData {
            accounts: ix.accounts,
            data: ix.data,
            program_id_index: program_id_index as u32,
        });

    let filtered_inner_ixs = inner_ixs
        .into_iter()
        .filter(|ix| ix.program_id_index == program_id_index as u32)
        .map(|ix| InstructionRawData {
            accounts: ix.accounts,
            data: ix.data,
            program_id_index: program_id_index as u32,
        });

    Ok(filtered_ixs.chain(filtered_inner_ixs).collect())
}
