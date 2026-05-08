use crate::*;
use futures::SinkExt;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::geyser::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterTransactions,
};

pub async fn send_subscription_request_grpc<T>(
    mut tx: T,
    subscribe_args: SubscribeRequestFilterTransactions,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: SinkExt<SubscribeRequest> + Unpin,
    <T as futures::Sink<SubscribeRequest>>::Error: std::error::Error + 'static,
{
    let mut accounts_filter = HashMap::new();
    accounts_filter.insert("account_monitor".to_string(), subscribe_args);

    tx.send(SubscribeRequest {
        transactions: accounts_filter,
        commitment: Some(CommitmentLevel::Processed as i32),
        ..Default::default()
    })
    .await?;

    Ok(())
}

pub struct GrpcClientConfig {
    pub grpc_endpoint: String,
    pub x_token: String,
    pub reconnect_delay_ms: u64,
}

impl GrpcClientConfig {
    pub fn new(grpc_endpoint: String, x_token: String) -> Self {
        Self {
            grpc_endpoint,
            x_token,
            reconnect_delay_ms: 1000,
        }
    }

    pub async fn setup_grpc_client(
        &self,
    ) -> Result<GeyserGrpcClient<impl Interceptor>, Box<dyn std::error::Error>> {
        let client = GeyserGrpcClient::build_from_shared(self.grpc_endpoint.clone())?
            .x_token(Some(self.x_token.clone()))?
            .tls_config(ClientTlsConfig::new().with_native_roots())?
            .connect()
            .await?;

        Ok(client)
    }

    pub async fn subscribe_with_reconnect(
        &self,
        subscribe_args: SubscribeRequestFilterTransactions,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut attempt = 0;

        loop {
            match self.process_migration_sniper_mode_stream(&subscribe_args).await {
                Ok(()) => {
                    info!("GRPC subscription completed successfully");
                    return Ok(());
                }
                Err(e) => {
                    attempt += 1;
                    error!("GRPC error on attempt {}: {:?}", attempt, e);

                    let delay = Duration::from_millis(self.reconnect_delay_ms);
                    info!("Reconnecting in {}ms...", delay.as_millis());
                    sleep(delay).await;
                }
            }
        }
    }

    async fn process_migration_sniper_mode_stream(
        &self,
        subscribe_args: &SubscribeRequestFilterTransactions,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut grpc_client = self.setup_grpc_client().await?;
        let (subscribe_tx, subscribe_rx) = grpc_client.subscribe().await?;
        send_subscription_request_grpc(subscribe_tx, subscribe_args.clone()).await?;
        Ok(process_stream(subscribe_rx).await?)
    }
}
