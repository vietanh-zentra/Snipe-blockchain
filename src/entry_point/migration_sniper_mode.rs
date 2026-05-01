use migration_sniper_bot::*;
use yellowstone_grpc_proto::geyser::SubscribeRequestFilterTransactions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //initialize zero slot http endpoint
    let _http_client = &*ZERO_SLOT_HTTP_CLIENT;

    // Start Telegram UI in background (single-user control panel)
    tokio::spawn(async {
        if let Err(e) = start_telegram_ui().await {
            eprintln!("Telegram UI failed: {e}");
        }
    });

    // Start non-blocking Telegram alert worker (single sender task + queue).
    if let Err(e) = start_telegram_alert_worker().await {
        eprintln!("Telegram alert worker failed to start: {e}");
    }

    // Fix #2/#4: Khởi tạo global alert bot cho anti-rug notifications
    migration_sniper_bot::modules::telegram_ui::alert_sender::init_alert_bot();

    tokio::spawn({
        async {
            loop {
                recent_blockhash_handler().await;
            }
        }
    });

    let grpc_config = GrpcClientConfig::new(GRPC_ENDPOINT.to_string(), GRPC_TOKEN.to_string());

    let subscribe_pumpfun_program_id = SubscribeRequestFilterTransactions {
        account_include: vec![],
        account_exclude: vec![],
        account_required: vec![PUMPSWAP_PROGRAM_ID.to_string()],
        vote: Some(false),
        failed: Some(false),
        signature: None,
    };

    // Exit process on Ctrl+C (main is blocked on grpc loop otherwise)
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            info!("Ctrl+C received, shutting down.");
        }
        std::process::exit(0);
    });

    if let Err(e) = grpc_config
        .subscribe_with_reconnect(subscribe_pumpfun_program_id)
        .await
    {
        return Err(e);
    }

    Ok(())
}
