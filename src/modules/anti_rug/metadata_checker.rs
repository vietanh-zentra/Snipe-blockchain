//! Module 5: Metadata Checker.
//!
//! Kiểm tra Metaplex on-chain metadata của token. Token hợp lệ thường
//! có URI trỏ tới website/social media. Token không có metadata → high-risk.
//!
//! Metaplex Metadata PDA:
//!   seeds = ["metadata", METAPLEX_PROGRAM_ID, mint_pubkey]
//!   program = metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s

use crate::RPC_CLIENT;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::timeout;

/// Metaplex Token Metadata program ID.
const METAPLEX_METADATA_PROGRAM: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";

pub struct MetadataCheckResult {
    /// True nếu account metadata tồn tại on-chain.
    pub metadata_account_exists: bool,
    /// True nếu URI field không rỗng.
    pub has_uri: bool,
    /// URI value (nếu có).
    pub uri: Option<String>,
    /// Token name (nếu parse được).
    pub name: Option<String>,
}

/// Derive Metaplex metadata PDA cho một mint.
fn derive_metadata_pda(mint: &Pubkey) -> Result<Pubkey, Box<dyn std::error::Error + Send + Sync>> {
    let program_id = Pubkey::from_str(METAPLEX_METADATA_PROGRAM)
        .map_err(|_| "Invalid Metaplex program ID")?;

    let seeds: &[&[u8]] = &[
        b"metadata",
        program_id.as_ref(),
        mint.as_ref(),
    ];

    let (pda, _bump) = Pubkey::find_program_address(seeds, &program_id);
    Ok(pda)
}

/// Kiểm tra metadata cho token mint.
pub async fn check_metadata(
    mint: &Pubkey,
    timeout_ms: u64,
) -> Result<Option<MetadataCheckResult>, Box<dyn std::error::Error + Send + Sync>> {
    let mint = *mint;
    let duration = Duration::from_millis(timeout_ms);

    let result = timeout(duration, async move {
        fetch_metadata(&mint).await
    })
    .await;

    match result {
        Ok(inner) => inner.map(Some),
        Err(_elapsed) => {
            eprintln!("[METADATA_CHECKER] Timeout after {}ms for mint {}", timeout_ms, mint);
            Ok(None)
        }
    }
}

async fn fetch_metadata(
    mint: &Pubkey,
) -> Result<MetadataCheckResult, Box<dyn std::error::Error + Send + Sync>> {
    let metadata_pda = derive_metadata_pda(mint)?;

    let account = match RPC_CLIENT.get_account(&metadata_pda).await {
        Ok(acc) => acc,
        Err(_) => {
            // Account không tồn tại → token không có metadata
            return Ok(MetadataCheckResult {
                metadata_account_exists: false,
                has_uri: false,
                uri: None,
                name: None,
            });
        }
    };

    // Parse metadata account data thủ công (tránh thêm dependency mpl-token-metadata)
    // Metaplex Metadata layout:
    //   1 byte: key (enum)
    //   32 bytes: update_authority
    //   32 bytes: mint
    //   4 + N bytes: name (length-prefixed string)
    //   4 + N bytes: symbol
    //   4 + N bytes: uri
    let data = &account.data;
    let result = parse_metadata_uri(data);

    Ok(result)
}

/// Parse Metaplex metadata binary data để lấy URI.
/// Format: key(1) + update_authority(32) + mint(32) + name(4+32) + symbol(4+10) + uri(4+200)
fn parse_metadata_uri(data: &[u8]) -> MetadataCheckResult {
    // Minimum offset để skip đến name field:
    // 1 (key) + 32 (update_authority) + 32 (mint) = 65
    let offset = 65_usize;

    if data.len() < offset + 4 {
        return MetadataCheckResult {
            metadata_account_exists: true,
            has_uri: false,
            uri: None,
            name: None,
        };
    }

    let mut pos = offset;

    // Parse name (length-prefixed, max 32 bytes padded with null bytes)
    let name = read_length_prefixed_string(data, &mut pos);

    // Parse symbol (max 10 bytes)
    let _symbol = read_length_prefixed_string(data, &mut pos);

    // Parse URI (max 200 bytes)
    let uri = read_length_prefixed_string(data, &mut pos);

    let has_uri = uri
        .as_ref()
        .map(|u| !u.trim_matches('\0').trim().is_empty())
        .unwrap_or(false);

    MetadataCheckResult {
        metadata_account_exists: true,
        has_uri,
        uri: uri.map(|u| u.trim_matches('\0').trim().to_string()),
        name: name.map(|n| n.trim_matches('\0').trim().to_string()),
    }
}

fn read_length_prefixed_string(data: &[u8], pos: &mut usize) -> Option<String> {
    if *pos + 4 > data.len() {
        return None;
    }
    let len = u32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]])
        as usize;
    *pos += 4;

    if *pos + len > data.len() {
        return None;
    }
    let bytes = &data[*pos..*pos + len];
    *pos += len;

    String::from_utf8(bytes.to_vec()).ok()
}

/// Kiểm tra metadata và trả về has_uri status.
pub async fn check_has_metadata(
    mint: &Pubkey,
    timeout_ms: u64,
) -> bool {
    match check_metadata(mint, timeout_ms).await {
        Ok(Some(result)) => result.has_uri,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_metadata_uri() {
        // Test với data quá ngắn
        let empty_data = vec![0u8; 10];
        let result = parse_metadata_uri(&empty_data);
        assert!(!result.has_uri);
    }

    #[test]
    fn test_derive_metadata_pda() {
        // Test rằng derive không panic
        let mint = Pubkey::new_unique();
        let pda = derive_metadata_pda(&mint);
        assert!(pda.is_ok());
    }
}
