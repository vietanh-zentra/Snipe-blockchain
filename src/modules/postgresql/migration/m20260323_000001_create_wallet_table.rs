use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Wallet::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Wallet::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Wallet::WalletId).integer().not_null())
                    .col(ColumnDef::new(Wallet::TelegramUserId).big_integer().not_null())
                    .col(ColumnDef::new(Wallet::PrivateKeyEncrypted).text().not_null())
                    .col(ColumnDef::new(Wallet::Pubkey).text().not_null())
                    .col(
                        ColumnDef::new(Wallet::IsSelected)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Wallet::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .index(
                        Index::create()
                            .name("idx-wallet-user-walletid-unique")
                            .col(Wallet::TelegramUserId)
                            .col(Wallet::WalletId)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Wallet::Table).if_exists().to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Wallet {
    Table,
    Id,
    WalletId,
    TelegramUserId,
    PrivateKeyEncrypted,
    Pubkey,
    IsSelected,
    CreatedAt,
}
