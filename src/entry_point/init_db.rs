use dotenvy::dotenv;
use migration_sniper_bot::{init_postgres_and_migrate, resolve_database_url_from_env};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = dotenv();
    let database_url = resolve_database_url_from_env()?;

    init_postgres_and_migrate(&database_url).await?;
    println!("✅ PostgreSQL migration complete. Tables are ready.");
    Ok(())
}
