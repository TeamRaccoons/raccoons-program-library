use anchor_lang::{prelude::*, solana_program::sysvar};
use bytemuck::{Pod, Zeroable};

declare_id!("SignedData111111111111111111111111111111112");

#[program]
pub mod signed_data {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, oracle_authority: Pubkey) -> Result<()> {
        ctx.accounts.config.set_inner(Config { oracle_authority });
        Ok(())
    }

    pub fn consume_signed_data(
        ctx: Context<ConsumeSignedData>,
        signature: [u8; 64],
        oracle_authority: Pubkey,
        oracle_data: OracleData,
    ) -> Result<()> {
        let current_instruction =
            sysvar::instructions::load_current_index_checked(&ctx.accounts.instructions_sysvar)?;
        require_neq!(current_instruction, 0);

        // The previous ix must be a ed25519 program instruction
        let ed25519_ix_index = (current_instruction - 1) as u8;
        let ed25519_ix = sysvar::instructions::load_instruction_at_checked(
            ed25519_ix_index as usize,
            &ctx.accounts.instructions_sysvar,
        )
        .map_err(|_| ProgramError::InvalidAccountData)?;

        // Check that the instruction is actually for the ed25519 verify program
        require_keys_eq!(
            ed25519_ix.program_id,
            anchor_lang::solana_program::ed25519_program::ID
        );

        pub const SIGNATURE_OFFSETS_SERIALIZED_SIZE: usize = 14;
        // bytemuck requires structures to be aligned
        pub const SIGNATURE_OFFSETS_START: usize = 2;
        let start = SIGNATURE_OFFSETS_START;
        let end = start.saturating_add(SIGNATURE_OFFSETS_SERIALIZED_SIZE);
        let offsets: &Ed25519SignatureOffsets =
            bytemuck::try_from_bytes(&ed25519_ix.data[start..end]).map_err(|e| {
                msg!("e: {}", e.to_string());
                ErrorCode::InvalidDataOffsets
            })?;
        require_eq!(offsets.signature_offset, 8);
        require_eq!(offsets.signature_instruction_index, current_instruction);
        require_eq!(offsets.public_key_offset, 8 + 64);
        require_eq!(offsets.public_key_instruction_index, current_instruction);
        require_eq!(offsets.message_data_offset, 8 + 64 + 32);
        require_eq!(offsets.message_data_size, OracleData::SIZE);
        require_eq!(offsets.message_instruction_index, current_instruction);

        require_keys_eq!(oracle_authority, ctx.accounts.config.oracle_authority);

        msg!(
            "Oracle data is signed by the oracle authority, price: {}",
            oracle_data.price
        );

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, seeds = [b"config"], bump, payer = payer, space = 8 + 32)]
    config: Account<'info, Config>,
    #[account(mut)]
    payer: Signer<'info>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ConsumeSignedData<'info> {
    #[account(seeds = [b"config"], bump)]
    config: Account<'info, Config>,
    /// CHECK: Address verified to be the instructions sysvar with load_instruction_at_checked
    instructions_sysvar: UncheckedAccount<'info>,
}

#[account]
pub struct Config {
    oracle_authority: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OracleData {
    pub sequence_id: u64,
    pub unix_timestamp: i64,
    pub price: u64,
    pub mint: Pubkey,
}

impl OracleData {
    const SIZE: u16 = 8 + 8 + 8 + 32;
}

#[error_code]
pub enum ErrorCode {
    #[msg("")]
    InvalidDataOffsets,
}

// Copied from solana monorepo to be accessible in program
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod, Eq, PartialEq)]
#[repr(C)]
pub struct Ed25519SignatureOffsets {
    pub signature_offset: u16, // offset to ed25519 signature of 64 bytes
    pub signature_instruction_index: u16, // instruction index to find signature
    pub public_key_offset: u16, // offset to public key of 32 bytes
    pub public_key_instruction_index: u16, // instruction index to find public key
    pub message_data_offset: u16, // offset to start of message data
    pub message_data_size: u16, // size of message data
    pub message_instruction_index: u16, // index of instruction data to get message data
}
