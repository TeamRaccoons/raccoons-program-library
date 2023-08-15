use anchor_lang::{
    prelude::*,
    solana_program::{
        hash::{hashv, Hash},
        instruction::Instruction,
        secp256k1_recover::secp256k1_recover,
        sysvar,
    },
};

declare_id!("Signed1nstruction11111111111111111111111112");

#[program]
pub mod signed_ix {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, authority: [u8; 64]) -> Result<()> {
        ctx.accounts.config.set_inner(Config { authority });
        Ok(())
    }

    /// The ix before must be signed by the authority
    pub fn verify_signed_ix(
        ctx: Context<VerifySignedIx>,
        signature: [u8; 64],
        recovery_id: u8,
        request: String,
        expire_at: i64,
    ) -> Result<()> {
        let current_ix_index =
            sysvar::instructions::load_current_index_checked(&ctx.accounts.instructions_sysvar)?;
        require_neq!(current_ix_index, 0);

        let current_ixn = sysvar::instructions::load_instruction_at_checked(
            current_ix_index as usize,
            &ctx.accounts.instructions_sysvar,
        )?;
        require_keys_eq!(current_ixn.program_id, *ctx.program_id); // This ensures it is a top level invocation as the runtime does not allow re-entrency

        // The previous ix must be a ed25519 program instruction
        let signed_ix_index = current_ix_index - 1;
        let signed_ix = sysvar::instructions::load_instruction_at_checked(
            signed_ix_index as usize,
            &ctx.accounts.instructions_sysvar,
        )
        .map_err(|_| ProgramError::InvalidAccountData)?;

        require_eq!(request, "https://whattodo.so/ix");
        require_gt!(expire_at, Clock::get()?.unix_timestamp);
        let message_hash = compute_message_hash(&request, expire_at, &signed_ix);

        verify_signature(
            signature,
            recovery_id,
            &ctx.accounts.config.authority,
            message_hash,
        )?;

        Ok(())
    }
}

pub fn compute_message_hash(request: &str, expire_at: i64, ix: &Instruction) -> Hash {
    let mut message_bytes = Vec::with_capacity(32 + (32 + 2) * ix.accounts.len() + ix.data.len());
    message_bytes.extend(request.as_bytes());
    message_bytes.extend(expire_at.to_le_bytes());
    message_bytes.extend(ix.program_id.to_bytes());
    for account in ix.accounts.iter() {
        message_bytes.extend(account.pubkey.as_ref());
        message_bytes.push(u8::from(account.is_signer));
        message_bytes.push(u8::from(account.is_writable));
    }
    message_bytes.extend(&ix.data);

    // It should cost 85 + 1 * message_bytes.len() cu to hash
    hashv(&[&message_bytes])
}

fn verify_signature(
    msg_signature: [u8; 64],
    recovery_id: u8,
    pubkey: &[u8; 64],
    message_hash: Hash,
) -> Result<()> {
    // Reject high-s value signatures to prevent malleability.
    {
        let signature = libsecp256k1::Signature::parse_standard_slice(&msg_signature)
            .map_err(|_| ProgramError::InvalidArgument)?;
        require!(
            !signature.s.is_high(),
            ErrorCode::EncounteredSignatureWithHighSValue
        ); // Encountered signature with high-s value"
    }

    let recovered_pubkey = secp256k1_recover(message_hash.as_ref(), recovery_id, &msg_signature)
        .map_err(|_| ProgramError::InvalidArgument)?;
    msg!("Signature recovered.");

    require!(
        &recovered_pubkey.0 == pubkey,
        ErrorCode::SignatureDoesNotMatchAuthority
    ); // Signature does not match authority

    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, seeds = [b"config"], bump, payer = payer, space = 8 + Config::INIT_SPACE)]
    config: Account<'info, Config>,
    #[account(mut)]
    payer: Signer<'info>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifySignedIx<'info> {
    #[account(seeds = [b"config"], bump)]
    config: Account<'info, Config>,
    /// CHECK: Address verified to be the instructions sysvar with load_instruction_at_checked
    instructions_sysvar: UncheckedAccount<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct Config {
    // a secp256k1 pubkey
    authority: [u8; 64],
}

#[error_code]
pub enum ErrorCode {
    EncounteredSignatureWithHighSValue,
    SignatureDoesNotMatchAuthority,
}
