use std::sync::Arc;

use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use permissioned_token;
use solana_program::{
    instruction::{AccountMeta, InstructionError},
    system_instruction,
};
use solana_program_test::{tokio::sync::Mutex, *};
use solana_sdk::{
    instruction::Instruction,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::{Transaction, TransactionError},
    transport::TransportError,
};
use spl_tlv_account_resolution::state::ExtraAccountMetas;
use spl_token_client::{
    client::{
        ProgramBanksClient, ProgramBanksClientProcessTransaction, ProgramClient, SendTransaction,
    },
    token::{ExtensionInitializationParams, Token, TokenError},
};
use spl_transfer_hook_interface::{
    get_extra_account_metas_address, instruction::initialize_extra_account_metas,
};

async fn setup(
    program_id: &Pubkey,
) -> (
    Arc<Mutex<ProgramTestContext>>,
    Arc<dyn ProgramClient<ProgramBanksClientProcessTransaction>>,
    Arc<Keypair>,
) {
    let mut program_test = ProgramTest::new("permissioned_token", *program_id, None);

    program_test.prefer_bpf(true);

    program_test.add_program("spl_token_2022", spl_token_2022::ID, None);

    let context = program_test.start_with_context().await;
    let payer = Arc::new(keypair_clone(&context.payer));
    let context = Arc::new(Mutex::new(context));

    let client: Arc<dyn ProgramClient<ProgramBanksClientProcessTransaction>> =
        Arc::new(ProgramBanksClient::new_from_context(
            Arc::clone(&context),
            ProgramBanksClientProcessTransaction,
        ));
    (context, client, payer)
}

async fn setup_mint<T: SendTransaction>(
    program_id: &Pubkey,
    mint_authority: &Pubkey,
    decimals: u8,
    payer: Arc<Keypair>,
    client: Arc<dyn ProgramClient<T>>,
) -> Token<T> {
    let mint_account = Keypair::new();
    let authority = payer.pubkey();
    let token = Token::new(
        client,
        program_id,
        &mint_account.pubkey(),
        Some(decimals),
        payer,
    );
    token
        .create_mint(
            mint_authority,
            None,
            vec![ExtensionInitializationParams::TransferHook {
                authority: Some(authority),
                program_id: Some(permissioned_token::ID),
            }],
            &[&mint_account],
        )
        .await
        .unwrap();
    token
}

fn keypair_clone(kp: &Keypair) -> Keypair {
    Keypair::from_bytes(&kp.to_bytes()).expect("failed to copy keypair")
}

#[tokio::test]
async fn test_permissioned_token() {
    let program_id = permissioned_token::ID;
    let (context, client, payer) = setup(&program_id).await;

    let token_program_id = spl_token_2022::id();
    let wallet = Keypair::new();
    let second_wallet = Keypair::new();
    let mint_authority = Keypair::new();
    let mint_authority_pubkey = mint_authority.pubkey();
    let decimals = 9;

    let permission_registry = Pubkey::find_program_address(
        &[permissioned_token::PERMISSION_REGISTRY_SEED],
        &permissioned_token::ID,
    )
    .0;

    let token = setup_mint(
        &token_program_id,
        &mint_authority_pubkey,
        decimals,
        payer.clone(),
        client.clone(),
    )
    .await;

    let extra_account_metas = get_extra_account_metas_address(token.get_address(), &program_id);

    token
        .create_associated_token_account(&wallet.pubkey())
        .await
        .unwrap();
    let source = token.get_associated_token_address(&wallet.pubkey());
    let token_amount = 1_000_000_000_000;
    token
        .mint_to(
            &source,
            &mint_authority_pubkey,
            token_amount,
            &[&mint_authority],
        )
        .await
        .unwrap();

    let destination = token.get_associated_token_address(&second_wallet.pubkey());
    token
        .create_associated_token_account(&second_wallet.pubkey())
        .await
        .unwrap();

    let extra_account_pubkeys = [AccountMeta::new_readonly(permission_registry, false)];
    {
        let mut context: tokio::sync::MutexGuard<ProgramTestContext> = context.lock().await;
        let rent = context.banks_client.get_rent().await.unwrap();
        let rent_lamports =
            rent.minimum_balance(ExtraAccountMetas::size_of(extra_account_pubkeys.len()).unwrap());
        let transaction = Transaction::new_signed_with_payer(
            &[
                system_instruction::transfer(
                    &context.payer.pubkey(),
                    &extra_account_metas,
                    rent_lamports,
                ),
                initialize_extra_account_metas(
                    &program_id,
                    &extra_account_metas,
                    token.get_address(),
                    &mint_authority_pubkey,
                    &extra_account_pubkeys,
                ),
            ],
            Some(&context.payer.pubkey()),
            &[&context.payer, &mint_authority],
            context.last_blockhash,
        );

        context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();

        let transaction = Transaction::new_signed_with_payer(
            &[Instruction {
                program_id: permissioned_token::ID,
                accounts: permissioned_token::accounts::Initialize {
                    authority: context.payer.pubkey(),
                    permission_registry,
                    system_program: system_program::ID,
                }
                .to_account_metas(None),
                data: permissioned_token::instruction::Initialize.data(),
            }],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            context.last_blockhash,
        );

        context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
    }

    // No permission
    {
        assert_eq!(
            token
                .transfer(&source, &destination, &wallet.pubkey(), 1, &[&wallet])
                .await
                .unwrap_err(),
            TokenError::Client(Box::new(TransportError::TransactionError(
                TransactionError::InstructionError(
                    0,
                    InstructionError::Custom(
                        permissioned_token::ErrorCode::MissingPermissionForSender.into()
                    )
                )
            )))
        );
    }

    // Add permission for sender
    {
        let mut context: tokio::sync::MutexGuard<ProgramTestContext> = context.lock().await;

        let transaction = Transaction::new_signed_with_payer(
            &[Instruction {
                program_id: permissioned_token::ID,
                accounts: permissioned_token::accounts::AddPermission {
                    authority: context.payer.pubkey(),
                    permission_registry,
                }
                .to_account_metas(None),
                data: permissioned_token::instruction::AddPermission {
                    owner: wallet.pubkey(),
                    allowed_send: true,
                    allowed_receive: true,
                    expire_at: i64::MAX,
                }
                .data(),
            }],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            context.last_blockhash,
        );

        context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
    }

    // No permission for receiver
    {
        assert_eq!(
            token
                .transfer(&source, &destination, &wallet.pubkey(), 1, &[&wallet])
                .await
                .unwrap_err(),
            TokenError::Client(Box::new(TransportError::TransactionError(
                TransactionError::InstructionError(
                    0,
                    InstructionError::Custom(
                        permissioned_token::ErrorCode::MissingPermissionForReceiver.into()
                    )
                )
            )))
        );
    }

    // Add permission for receiver
    {
        let mut context: tokio::sync::MutexGuard<ProgramTestContext> = context.lock().await;

        let transaction = Transaction::new_signed_with_payer(
            &[Instruction {
                program_id: permissioned_token::ID,
                accounts: permissioned_token::accounts::AddPermission {
                    authority: context.payer.pubkey(),
                    permission_registry,
                }
                .to_account_metas(None),
                data: permissioned_token::instruction::AddPermission {
                    owner: second_wallet.pubkey(),
                    allowed_send: true,
                    allowed_receive: true,
                    expire_at: i64::MAX,
                }
                .data(),
            }],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            context.last_blockhash,
        );

        context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
    }

    // All permissions
    {
        token
            .transfer(&source, &destination, &wallet.pubkey(), 1, &[&wallet])
            .await
            .unwrap();
    }
}
