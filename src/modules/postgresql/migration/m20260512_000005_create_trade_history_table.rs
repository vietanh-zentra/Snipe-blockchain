use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TradeHistory::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TradeHistory::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TradeHistory::TokenMint)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeHistory::TradeType)
                            .string_len(10)
                            .not_null(), // "buy" or "sell"
                    )
                    .col(
                        ColumnDef::new(TradeHistory::SolAmount)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeHistory::TokenPrice)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeHistory::McAtTrade)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .col(
                        ColumnDef::new(TradeHistory::TxHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TradeHistory::Status)
                            .string_len(16)
                            .not_null(), // "success" or "failed"
                    )
                    .col(
                        ColumnDef::new(TradeHistory::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for fast time-range queries
        manager
            .create_index(
                Index::create()
                    .name("idx_trade_history_created_at")
                    .table(TradeHistory::Table)
                    .col(TradeHistory::CreatedAt)
                    .to_owned(),
            )
            .await?;

        // Index for mint lookups (PNL per token)
        manager
            .create_index(
                Index::create()
                    .name("idx_trade_history_token_mint")
                    .table(TradeHistory::Table)
                    .col(TradeHistory::TokenMint)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TradeHistory::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum TradeHistory {
    Table,
    Id,
    TokenMint,
    TradeType,
    SolAmount,
    TokenPrice,
    McAtTrade,
    TxHash,
    Status,
    CreatedAt,
}
