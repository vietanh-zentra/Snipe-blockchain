use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SkippedTokensLog::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SkippedTokensLog::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SkippedTokensLog::TokenMint)
                            .string_len(44)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SkippedTokensLog::RejectionReason)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SkippedTokensLog::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for fast query by token_mint
        manager
            .create_index(
                Index::create()
                    .name("idx-skipped-tokens-log-token-mint")
                    .table(SkippedTokensLog::Table)
                    .col(SkippedTokensLog::TokenMint)
                    .to_owned(),
            )
            .await?;

        // Index for time-based queries
        manager
            .create_index(
                Index::create()
                    .name("idx-skipped-tokens-log-timestamp")
                    .table(SkippedTokensLog::Table)
                    .col(SkippedTokensLog::Timestamp)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(SkippedTokensLog::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SkippedTokensLog {
    Table,
    Id,
    TokenMint,
    RejectionReason,
    Timestamp,
}
