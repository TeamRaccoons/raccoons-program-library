//! Structs required to verify spl-token-2022 mints.
//!
//! By copying the required functions here, we avoid a circular dependency
//! between spl-token-2022 and this crate.

use {
    arrayref::{array_ref, array_refs},
    solana_program::{program_error::ProgramError, program_option::COption, pubkey::Pubkey},
};

fn unpack_coption_key(src: &[u8; 36]) -> Result<COption<Pubkey>, ProgramError> {
    let (tag, body) = array_refs![src, 4, 32];
    match *tag {
        [0, 0, 0, 0] => Ok(COption::None),
        [1, 0, 0, 0] => Ok(COption::Some(Pubkey::new_from_array(*body))),
        _ => Err(ProgramError::InvalidAccountData),
    }
}

/// Extract the mint authority from the account bytes
pub fn get_mint_authority(account_data: &[u8]) -> Result<COption<Pubkey>, ProgramError> {
    const MINT_SIZE: usize = 82;
    if account_data.len() < MINT_SIZE {
        Err(ProgramError::InvalidAccountData)
    } else {
        let mint_authority = array_ref![account_data, 0, 36];
        unpack_coption_key(mint_authority)
    }
}

/// Extract the owner from the account bytes
pub fn get_account_owner(account_data: &[u8]) -> Result<Pubkey, ProgramError> {
    const ACCOUNT_SIZE: usize = 165;
    if account_data.len() < ACCOUNT_SIZE {
        Err(ProgramError::InvalidAccountData)
    } else {
        let owner = array_ref![account_data, 32, 32];
        Ok(Pubkey::new(owner))
    }
}
