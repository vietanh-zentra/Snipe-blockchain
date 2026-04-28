use once_cell::sync::Lazy;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::commitment_config::CommitmentLevel;

pub static ZERO_SLOT_HTTP_CLIENT: Lazy<Arc<Client>> = Lazy::new(|| {
    println!("🔄 Initializing 0-slot HTTP client...");
    
    let client = Client::builder()
        .pool_idle_timeout(Duration::from_secs(300))     // 5 minutes
        .pool_max_idle_per_host(5)                       // Multiple connections
        .tcp_keepalive(Duration::from_secs(10))          // Frequent keep-alive
        .tcp_nodelay(true)                               // MUST for low latency
        .connect_timeout(Duration::from_secs(3))         // Fast connection
        .timeout(Duration::from_secs(10))                // Reasonable timeout
        .http2_keep_alive_interval(Duration::from_secs(20))
        .http2_keep_alive_timeout(Duration::from_secs(90))
        .http2_keep_alive_while_idle(true)
        .use_rustls_tls()                                // Faster TLS
        .build()
        .expect("Failed to build 0-slot HTTP client");
    
    // PRE-WARM THIS SPECIFIC ENDPOINT
    let client_arc = Arc::new(client);
    pre_warm_zero_slot_endpoint(client_arc.clone());
    
    client_arc
});

pub fn pre_warm_zero_slot_endpoint(client: Arc<Client>) {
    tokio::spawn(async move {
        println!("🔥 Pre-warming 0-slot endpoint...");
        
        // Try multiple times to establish connection
        for attempt in 1..=3 {
            let url = "http://de2.0slot.trade?api-key=335e371309b6492584368e9dc553622d".to_string();
            
            match client.get(&url).send().await {
                Ok(response) => {
                    println!("✅ 0-slot endpoint ready (attempt {}): HTTP {}", 
                        attempt, response.status());
                    
                    // If it's a 404 or similar, that's OK - connection is established
                    if response.status().is_success() {
                        println!("🎯 Successfully connected to 0-slot service");
                    }
                    break;
                }
                Err(e) if attempt < 3 => {
                    println!("⚠️ 0-slot warm-up attempt {} failed: {:?}", attempt, e);
                    tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                }
                Err(e) => {
                    eprintln!("❌ Failed to pre-warm 0-slot endpoint: {:?}", e);
                }
            }
        }
    });
}

fn connection_rpc() -> String {
    std::env::var("RPC_ENDPOINT").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string())
}

fn grpc_endpoint() -> String {
    std::env::var("GRPC_ENDPOINT").unwrap_or_else(|_| "https://grpc.mainnet.solana.com".to_string())
}

fn grpc_token() -> String {
    std::env::var("GRPC_TOKEN").unwrap_or_default()
}

pub static RPC_ENDPOINT: Lazy<String> = Lazy::new(connection_rpc);
pub static GRPC_ENDPOINT: Lazy<String> = Lazy::new(grpc_endpoint);
pub static GRPC_TOKEN: Lazy<String> = Lazy::new(grpc_token);
pub static RPC_CLIENT: Lazy<Arc<RpcClient>> = Lazy::new(|| {
    Arc::new(RpcClient::new_with_commitment(
        connection_rpc(),
        CommitmentConfig {
            commitment: CommitmentLevel::Processed,
        },
    ))
});