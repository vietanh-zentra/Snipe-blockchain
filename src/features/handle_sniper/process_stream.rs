use crate::*;
use futures::StreamExt;
use yellowstone_grpc_proto::{geyser::SubscribeUpdate, tonic::Status};

pub async fn process_stream<S>(mut stream: S) -> Result<(), Box<dyn std::error::Error>>
where
    S: StreamExt<Item = Result<SubscribeUpdate, Status>> + Unpin,
{
    while let Some(result) = stream.next().await {
        match result {
            Ok(update) => {
                if !is_running() {
                    continue;
                }

                let (account_keys, ixs, inner_ixs, tx_id, _signers) =
                    if let Some(data) = extract_transaction_data(&update) {
                        data
                    } else {
                        continue;
                    };

                let ix_info_pumpfun =
                    filter_by_program_id(ixs.clone(), inner_ixs.clone(), PUMPFUN_PROGRAM_ID, account_keys.clone())
                        .unwrap();

                let ix_info_pumpswap = match filter_by_program_id(
                    ixs.clone(),
                    inner_ixs.clone(),
                    PUMPSWAP_PROGRAM_ID,
                    account_keys.clone(),
                ) {
                    Ok(data) => data,
                    Err(_) => {
                        vec![]
                    }
                };

                let mut all_pump_ix = vec![];
                all_pump_ix.extend(ix_info_pumpfun.clone());
                all_pump_ix.extend(ix_info_pumpswap.clone());

                let migration_data = migrate_info(all_pump_ix.clone(), account_keys.clone(), &tx_id);

                let pumpswap_trade_data =
                    get_pumpswap_trade_info(ix_info_pumpswap.clone(), account_keys.clone(), &tx_id);

                let trade_token_data_map = handle_trade_events(migration_data, pumpswap_trade_data, tx_id.clone()).await;
                execute_trade(&trade_token_data_map).await;
            }

            Err(e) => {
                log!("Stream error: {}", e);
                return Err(Box::new(e));
            }
        }
    }

    Ok(())
}
