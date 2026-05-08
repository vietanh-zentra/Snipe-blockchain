use crate::*;
use base64;
use serde_json::json;
#[allow(deprecated)]
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, instruction::Instruction, pubkey::Pubkey,
    signer::keypair::Keypair,
    signer::Signer,
    system_instruction, transaction::Transaction,
};
use std::str::FromStr;
use std::time::Instant;
use crate::BOT_RUN_STATE;

#[allow(deprecated)]
pub async fn send_0slot_transaction(
    raw_instructions: Vec<Instruction>,
    keypair: Keypair,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let start_time = Instant::now();

    let run_state = BOT_RUN_STATE.read().await.clone();
    let cu: u32 = default_cu();
    let third_party_fee = run_state.trading.tip_fee_sol;
    let priority_fee_micro_lamport = run_state.trading.priority_fee_micro_lamports;
    let signer_pubkey = keypair.pubkey();

    let mut total_instruction = Vec::new();

    //budget compute unit limit
    total_instruction.push(ComputeBudgetInstruction::set_compute_unit_limit(cu as u32));
    //compute unit price
    total_instruction.push(ComputeBudgetInstruction::set_compute_unit_price(
        priority_fee_micro_lamport,
    ));

    //pure ix
    total_instruction.extend(raw_instructions);

    //tip ix
    let tip_receiver = Pubkey::from_str("TpdxgNJBWZRL8UXF5mrEsyWxDWx9HQexA9P1eTWQ42p").unwrap();
    let tip_transfer_instruction = system_instruction::transfer(
        &signer_pubkey,                           // Sender's public key
        &tip_receiver,                            // Tip receiver's public key
        (third_party_fee * 10f64.powi(9)) as u64, // Amount to transfer as a tip (0.001 SOL in this case)
    );

    total_instruction.push(tip_transfer_instruction);

    let mut transaction = Transaction::new_with_payer(&total_instruction, Some(&signer_pubkey));

    // Sign the transaction with the sender's keypair
    transaction
        .try_sign(&[keypair.insecure_clone()], get_slot())
        .expect("Failed to sign transaction");

    let serialized_transaction = bincode::serialize(&transaction).unwrap();
    let base64_encoded_transaction = base64::encode(serialized_transaction);

    // Build the JSON-RPC request
    let request_body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sendTransaction",
        "params": [
            base64_encoded_transaction,
            {
                "encoding": "base64",
                "skipPreflight": true,
            }
        ]
    });

    println!("TX making: {:?}", start_time.elapsed());
    let tx_submission_start = Instant::now();

    let response = ZERO_SLOT_HTTP_CLIENT
        .post("http://de2.0slot.trade?api-key=335e371309b6492584368e9dc553622d")
        .json(&request_body)
        .send()
        .await?;
    let response_json: serde_json::Value = response.json().await?;

    if let Some(result) = response_json.get("result") {
        let hash_str = result.as_str().unwrap_or("").to_string();
        info!(
            "[✔ SUBMIT]
            \tTransaction(zero slot) submission took: {:?}
            \t* Service: ZERO_SLOT
            \t* Hash: {:?}",
            tx_submission_start.elapsed(),
            result,
        );
        Ok(Some(hash_str))
    } else if let Some(error) = response_json.get("error") {
        eprintln!("Failed to send transaction: {}", error);
        Ok(None)
    } else {
        Ok(None)
    }
}

