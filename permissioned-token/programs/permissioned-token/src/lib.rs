use anchor_lang::prelude::*;

use anchor_lang::solana_program::{
    account_info::AccountInfo, program_error::PrintProgramError, pubkey::Pubkey,
};
use spl_transfer_hook_interface::error::TransferHookError;

mod inline_spl_token;
mod processor;

pub const PERMISSION_REGISTRY_SEED: &[u8] = b"permission-registry";

declare_id!("PermissionedToken11111111111111111111111112");

#[program]
pub mod permissioned_token {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts
            .permission_registry
            .set_inner(PermissionRegistry {
                authority: ctx.accounts.authority.key(),
                permissions: vec![],
            });
        Ok(())
    }

    pub fn add_permission(
        ctx: Context<AddPermission>,
        allowed_send: bool,
        allowed_receive: bool,
        expire_at: i64,
        owner: Pubkey,
    ) -> Result<()> {
        let permissions = &mut ctx.accounts.permission_registry.permissions;
        for permission in permissions.iter() {
            require_keys_neq!(permission.owner, owner);
        }
        permissions.push(Permission {
            owner,
            allowed_send,
            allowed_receive,
            expire_at,
        });
        Ok(())
    }

    pub fn update_permission(
        ctx: Context<AddPermission>,
        allowed_send: bool,
        allowed_receive: bool,
        expire_at: i64,
        owner: Pubkey,
    ) -> Result<()> {
        let permissions = &mut ctx.accounts.permission_registry.permissions;
        let permission = permissions
            .iter_mut()
            .find(|permission| permission.owner == owner)
            .ok_or(ErrorCode::MissingPermission)?;

        permission.allowed_send = allowed_send;
        permission.allowed_receive = allowed_receive;
        permission.expire_at = expire_at;

        Ok(())
    }

    // TODO: delete permission

    /// The fallback allows routing methods to match the transfer hook interface
    pub fn fallback(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> Result<()> {
        if let Err(error) = processor::process(program_id, accounts, instruction_data) {
            // catch the error so we can print it
            error.print::<TransferHookError>();
            return Err(error.into());
        }

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, seeds = [PERMISSION_REGISTRY_SEED], bump, payer = authority, space = PermissionRegistry::SPACE)]
    pub permission_registry: Account<'info, PermissionRegistry>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddPermission<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut, has_one = authority)]
    pub permission_registry: Account<'info, PermissionRegistry>,
}

#[account]
pub struct PermissionRegistry {
    pub authority: Pubkey,
    /// TODO: Zero copy extendable array storage
    pub permissions: Vec<Permission>,
}

impl PermissionRegistry {
    const SPACE: usize = 8 + 32 + 4 + 10 * Permission::SPACE;

    /// Validate that both sender and receiver have necessary permissions
    fn validate_transfer(&self, sender: &Pubkey, receiver: &Pubkey) -> Result<()> {
        let mut allowed_send = false;
        let mut allowed_receive = false;
        for permission in self.permissions.iter() {
            if &permission.owner == sender {
                require!(
                    permission.allowed_send,
                    ErrorCode::MissingPermissionForSender
                );
                allowed_send = true;
            } else if &permission.owner == receiver {
                require!(
                    permission.allowed_receive,
                    ErrorCode::MissingPermissionForReceiver
                );
                allowed_receive = true;
            }
            if allowed_send && allowed_receive {
                return Ok(());
            }
        }

        if !allowed_send {
            return Err(ErrorCode::MissingPermissionForSender.into());
        }
        if !allowed_receive {
            return Err(ErrorCode::MissingPermissionForReceiver.into());
        }

        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Permission {
    pub owner: Pubkey,
    pub allowed_send: bool,
    pub allowed_receive: bool,
    pub expire_at: i64,
}

impl Permission {
    const SPACE: usize = 32 + 2 + 8;
}

#[error_code]
pub enum ErrorCode {
    MissingPermission,
    MissingPermissionForSender,
    MissingPermissionForReceiver,
}
