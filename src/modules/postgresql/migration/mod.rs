use sea_orm_migration::prelude::*;

mod m20260323_000001_create_wallet_table;
mod m20260323_000002_create_trading_parameter_table;
mod m20260430_000003_create_anti_rug_log_table;
mod m20260508_000004_create_skipped_tokens_log_table;
mod m20260512_000005_create_trade_history_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260323_000001_create_wallet_table::Migration),
            Box::new(m20260323_000002_create_trading_parameter_table::Migration),
            Box::new(m20260430_000003_create_anti_rug_log_table::Migration),
            Box::new(m20260508_000004_create_skipped_tokens_log_table::Migration),
            Box::new(m20260512_000005_create_trade_history_table::Migration),
        ]
    }
}
