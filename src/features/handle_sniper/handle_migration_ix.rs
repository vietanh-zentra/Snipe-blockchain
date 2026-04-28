use crate::*;
use colored::*;
use dashmap::DashMap;
use solana_sdk::pubkey::Pubkey;

pub async fn handle_trade_events(
    migration_data: (
        Vec<MigrateInstructionAccounts>,
        Vec<CreatePoolInstructionAccounts>,
        Vec<CreatePoolEventData>,
        Vec<CreatePoolInstructionData>,
    ),
    pumpswap_trade_data: (
        Vec<PumpswapBuyEvent>,
        Vec<PumpswapSellEvent>,
        Vec<PumpswapBuyInstructionAccounts>,
        Vec<PumpswapSellInstructionAccounts>,
    ),
    tx_id: String,
) -> DashMap<Pubkey, TokenDatabaseSchema> {
    //Migration data
    let (
        migrate_instruction_accounts,
        create_pool_instruction_accounts,
        create_pool_event_data,
        create_pool_instruction_data,
    ) = migration_data;

    //Pumpswap trade data
    let (
        pumpswap_buy_events,
        pumpswap_sell_events,
        pumpswap_buy_instruction_accounts,
        pumpswap_sell_instruction_accounts,
    ) = pumpswap_trade_data;

    let return_data: DashMap<Pubkey, TokenDatabaseSchema> = DashMap::new();

    //handle migration instructions
    if migrate_instruction_accounts.len() != create_pool_event_data.len()
        || migrate_instruction_accounts.len() != create_pool_instruction_accounts.len()
        || migrate_instruction_accounts.len() != create_pool_instruction_data.len()
    {
        log!("[POOL CREATION] ---- *Tx: {}", tx_id);
    }

    let migration_count = migrate_instruction_accounts
        .len()
        .min(create_pool_instruction_accounts.len())
        .min(create_pool_event_data.len())
        .min(create_pool_instruction_data.len());

    for i in 0..migration_count {
        if !create_pool_event_data[i].is_mayhem_mode {
            info!(
                "[MIGRATION]\t*Mint: {}\t*Tx: {}",
                create_pool_event_data[i].base_mint.to_string().green(),
                solscan!(tx_id.to_string().purple())
            );

            enqueue_migration_detected_alert(
                &create_pool_event_data[i].base_mint.to_string(),
                &tx_id,
            );

            let token_data = TokenDatabaseSchema::new_from_token_migration(
                migrate_instruction_accounts[i].clone(),
                create_pool_instruction_accounts[i].clone(),
                create_pool_event_data[i].clone(),
                create_pool_instruction_data[i].clone(),
            );
            return_data.insert(token_data.token_mint, token_data);
        }
    }

    //handle pumpswap instructions
    for (i, pumpswap_buy_event) in pumpswap_buy_events.iter().enumerate() {
        if let Some(token_data) = TOKEN_DB
            .get(pumpswap_buy_instruction_accounts[i].base_mint)
            .unwrap()
        {
            let updated_token_data = update_status_from_pumpswap_buy_event(
                token_data.clone(),
                pumpswap_buy_event.clone(),
                pumpswap_buy_instruction_accounts[i].clone(),
                tx_id.to_string(),
            );
            return_data.insert(updated_token_data.token_mint, updated_token_data);
        }
    }

    for (i, pumpswap_sell_event) in pumpswap_sell_events.iter().enumerate() {
        if let Some(token_data) = TOKEN_DB
            .get(pumpswap_sell_instruction_accounts[i].base_mint)
            .unwrap()
        {
            let updated_token_data = update_status_from_pumpswap_sell_event(
                token_data,
                pumpswap_sell_event.clone(),
                pumpswap_sell_instruction_accounts[i].clone(),
                tx_id.clone(),
            );

            if let Some(updated_data) = updated_token_data {
                return_data.insert(updated_data.token_mint, updated_data);
            }
        }
    }

    return_data
}
