use crate::*;
use solana_sdk::pubkey::Pubkey;

pub fn get_pumpswap_user_volume_accumulator(wallet_address: Pubkey) -> Pubkey {
    let (pda, _bump) = Pubkey::find_program_address(
        &[USER_VOLUME_ACCUMULATOR_SEED, wallet_address.as_ref()],
        &PUMPSWAP_PROGRAM_ID,
    );
    pda
}