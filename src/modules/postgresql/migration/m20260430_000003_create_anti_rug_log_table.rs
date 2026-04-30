use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AntiRugFilterLog::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AntiRugFilterLog::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AntiRugFilterLog::TokenMint)
                            .string_len(44)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AntiRugFilterLog::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        // 'pass' | 'fail' | 'warn'
                        ColumnDef::new(AntiRugFilterLog::Verdict)
                            .string_len(10)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AntiRugFilterLog::RejectReason)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(AntiRugFilterLog::Top10HolderPct)
                            .double()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(AntiRugFilterLog::DevTxCount)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(AntiRugFilterLog::GenesisBuyPct)
                            .double()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(AntiRugFilterLog::GenesisBundleDetected)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(AntiRugFilterLog::HasMetadataUri)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(AntiRugFilterLog::FilterDurationMs)
                            .big_integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for fast query by mint
        manager
            .create_index(
                Index::create()
                    .name("idx-anti-rug-log-token-mint")
                    .table(AntiRugFilterLog::Table)
                    .col(AntiRugFilterLog::TokenMint)
                    .to_owned(),
            )
            .await?;

        // Index for time-based queries
        manager
            .create_index(
                Index::create()
                    .name("idx-anti-rug-log-created-at")
                    .table(AntiRugFilterLog::Table)
                    .col(AntiRugFilterLog::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(AntiRugFilterLog::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum AntiRugFilterLog {
    Table,
    Id,
    TokenMint,
    CreatedAt,
    Verdict,
    RejectReason,
    Top10HolderPct,
    DevTxCount,
    GenesisBuyPct,
    GenesisBundleDetected,
    HasMetadataUri,
    FilterDurationMs,
}
