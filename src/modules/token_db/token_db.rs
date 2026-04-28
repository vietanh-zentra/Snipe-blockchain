use super::TokenDatabaseSchema;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::Arc;
use solana_sdk::pubkey::Pubkey;

type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub static TOKEN_DB: Lazy<TokenDB> = Lazy::new(|| TokenDB {
    map: Arc::new(DashMap::new()),
});

#[derive(Clone)]
pub struct TokenDB {
    map: Arc<DashMap<Pubkey, TokenDatabaseSchema>>,
}

impl TokenDB {
    /// Create a new empty TokenStoreHalfCopy
    pub fn new() -> Self {
        Self {
            map: Arc::new(DashMap::new()),
        }
    }

    /// Insert or update a TokenDatabaseSchema entry
    pub fn upsert(&self, key: Pubkey, data: TokenDatabaseSchema) -> Result<(), BoxError> {
        self.map.insert(key, data);
        Ok(())
    }

    /// Read a TokenDatabaseSchema entry by key
    pub fn get(&self, key: Pubkey) -> Result<Option<TokenDatabaseSchema>, BoxError> {
        Ok(self.map.get(&key).map(|v| v.clone()))
    }

    /// Delete a TokenDatabaseSchema entry by key
    pub fn delete(&self, key: Pubkey) -> Result<(), BoxError> {
        self.map.remove(&key);
        Ok(())
    }

    /// List all key-value pairs
    pub fn list_all(&self) -> Result<Vec<(Pubkey, TokenDatabaseSchema)>, BoxError> {
        let mut results = Vec::new();
        for r in self.map.iter() {
            results.push((r.key().clone(), r.value().clone()));
        }
        Ok(results)
    }
}