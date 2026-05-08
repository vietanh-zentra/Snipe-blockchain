use solana_sdk::{pubkey, pubkey::Pubkey};

pub const WSOL: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

pub const MAYHEM_FEE_RECIPIENT: Pubkey = pubkey!("GesfTA3X2arioaHp8bbKdjG9vJtskViWACZoYvxp4twS");

//token program address
pub const ASSOCIATED_PROGRAM: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const TOKEN_2022_PROGRAM: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

//seeds
pub static USER_VOLUME_ACCUMULATOR_SEED: &[u8] = b"user_volume_accumulator";
pub static GLOBAL_ACCUMULATOR_SEED: &[u8] = b"global_volume_accumulator";
pub static PUMPFUN_CREATOR_VAULT_SEED: &[u8] = b"creator-vault";
pub static PUMPSWAP_CREATOR_VAULT_SEED: &[u8] = b"creator_vault";
pub static POOL_SEED: &[u8] = b"pool";
pub static PUMPFUN_BONDING_CURVE: &[u8] = b"bonding-curve";
pub static PUMPFUN_BONDING_CURVE_V2_SEED: &[u8] = b"bonding-curve-v2";
pub static PUMPFUN_POOL_AUTH: &[u8] = b"pool-authority";
pub static PUMPSWAP_POOL_V2_SEED: &[u8] = b"pool-v2";

//fee program
pub const BUDGET_COMPUTE_PROGRAM: Pubkey = pubkey!("ComputeBudget111111111111111111111111111111");
//system program
pub const SYSTEM_PROGRAM: Pubkey = pubkey!("11111111111111111111111111111111");

//pumpfun accounts
pub const PUMPFUN_PROGRAM_ID: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");
pub const PUMPFUN_FEE_RECIPIENT: Pubkey = pubkey!("62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV");
pub const PUMPFUN_FEE_CONFIG: Pubkey = pubkey!("8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt");
pub const PUMPFUN_FEE_PROGRAM: Pubkey = pubkey!("pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ");
pub const PUMPFUN_MINT_AUTHORITY: Pubkey = pubkey!("TSLvdd1pWpHVjahSpsvCXUbgwsL3JAcvokwaKt1eokM");
pub const PUMPFUN_GLOBAL: Pubkey = pubkey!("4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf");
pub const PUMP_FUN_EVENT_AUTHORITY: Pubkey =
    pubkey!("Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1");
pub const PUMPFUN_GLOBAL_VOLUME_ACCUMULATOR: Pubkey =
    pubkey!("Hq2wp8uJ9jCPsYgNHex8RtqdvMPfVGoYwjvF1ATiwn2Y");
pub const PUMPFUN_MIGRATE_PROGRAM: Pubkey = pubkey!("39azUYFWPz3VHgKCf3VChUwbpURdCHRxjWVowf5jUJjg");

//pumpswap accounts
pub const PUMPSWAP_PROGRAM_ID: Pubkey = pubkey!("pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA");
pub const PUMPSWAP_GLOBAL: Pubkey = pubkey!("ADyA8hdefvWN2dbGGWFotbzWxrAvLW83WG6QCVXvJKqw");
pub const PUMPSWAP_FEE_1: Pubkey = pubkey!("62qc2CNXwrYqQScmEdiZFFAnJR262PxWEuNQtxfafNgV");
pub const PUMPSWAP_EVENT_AUTH: Pubkey = pubkey!("GS4CU59F31iL7aR2Q8zVS8DRrcRnXX1yjQ66TqNVQnaR");
pub const PUMPSWAP_FEE_CONFIG: Pubkey = pubkey!("5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx");
pub const PUMPSWAP_FEE_PROGRAM: Pubkey = pubkey!("pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ");

pub const PUMPSWAP_GLOBAL_VOLUME_ACCUMULATOR: Pubkey =
    pubkey!("C2aFPdENg4A2HQsmrd5rTw5TaYBX5Ku887cWjbFKtZpw");

//mayhem
pub const MAYHEM_PROTOCOL_FEE_RECIPIENT: Pubkey =
    pubkey!("GesfTA3X2arioaHp8bbKdjG9vJtskViWACZoYvxp4twS");

//buyback fee recipients (required by PumpSwap AMM update - fixes error 6058)
//official list from https://github.com/pump-fun/pump-public-docs/blob/main/docs/FEE_RECIPIENTS.md
pub const PUMPSWAP_BUYBACK_FEE_RECIPIENT: Pubkey =
    pubkey!("5YxQFdt3Tr9zJLvkFccqXVUwhdTWJQc1fFg2YPbxvxeD");
