use anchor_lang::{prelude::Pubkey, AnchorSerialize, InstructionData, ToAccountMetas};
// use ed25519_dalek::Signer;
use rand::thread_rng;
use signed_data;
use solana_program_test::*;
use solana_sdk::{
    ed25519_instruction::new_ed25519_instruction,
    instruction::{Instruction, InstructionError},
    signature::Keypair,
    signer::Signer,
    system_program, sysvar,
    transaction::Transaction,
    transport::TransportError,
};
mod ed25519_helper;
use solana_sdk::{pubkey, transaction::TransactionError};

pub async fn process_transaction(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> std::result::Result<(), BanksClientError> {
    let recent_blockhash = context.banks_client.get_latest_blockhash().await?;

    let mut all_signers = vec![&context.payer];
    all_signers.extend_from_slice(signers);

    let transaction = Transaction::new_signed_with_payer(
        &instructions,
        Some(&context.payer.pubkey()),
        &all_signers,
        recent_blockhash,
    );

    context.banks_client.process_transaction(transaction).await
}

#[tokio::test]
async fn test_consume_signed_data() {
    let pt = ProgramTest::new("signed_data", signed_data::ID, None);

    let oracle_authority_keypair = ed25519_dalek::Keypair::generate(&mut thread_rng());
    let oracle_authority = Pubkey::try_from(oracle_authority_keypair.public.as_ref()).unwrap();
    let usdc_mint = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    let config = Pubkey::find_program_address(&[b"config"], &signed_data::ID).0;

    let mut context = pt.start_with_context().await;
    let payer = context.payer.pubkey();

    process_transaction(
        &mut context,
        &[Instruction {
            program_id: signed_data::ID,
            accounts: signed_data::accounts::Initialize {
                config,
                payer,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: signed_data::instruction::Initialize { oracle_authority }.data(),
        }],
        &[],
    )
    .await
    .unwrap();

    let oracle_data = signed_data::OracleData {
        sequence_id: 100,
        unix_timestamp: 13456,
        price: 987654321,
        mint: usdc_mint,
    };
    let message = oracle_data.try_to_vec().unwrap();
    let signature = ed25519_dalek::Signer::try_sign(&oracle_authority_keypair, &message).unwrap();

    process_transaction(
        &mut context,
        &[
            ed25519_helper::new_ed25519_instruction_without_payload(
                &oracle_authority_keypair,
                &message,
                1,
                8,
            ),
            Instruction {
                program_id: signed_data::ID,
                accounts: signed_data::accounts::ConsumeSignedData {
                    config,
                    instructions_sysvar: sysvar::instructions::ID,
                }
                .to_account_metas(None),
                data: signed_data::instruction::ConsumeSignedData {
                    signature: signature.to_bytes(),
                    oracle_authority,
                    oracle_data: oracle_data.clone(),
                }
                .data(),
            },
        ],
        &[],
    )
    .await
    .unwrap();

    // Same message but signature is incorrect
    let mut signature = signature.to_bytes();
    signature[0] += 1; // Screw up the signature
    let result = process_transaction(
        &mut context,
        &[
            ed25519_helper::new_ed25519_instruction_without_payload(
                &oracle_authority_keypair,
                &message,
                1,
                8,
            ),
            Instruction {
                program_id: signed_data::ID,
                accounts: signed_data::accounts::ConsumeSignedData {
                    config,
                    instructions_sysvar: sysvar::instructions::ID,
                }
                .to_account_metas(None),
                data: signed_data::instruction::ConsumeSignedData {
                    signature,
                    oracle_authority,
                    oracle_data,
                }
                .data(),
            },
        ],
        &[],
    )
    .await;
    assert_eq!(
        result.unwrap_err().unwrap(),
        TransactionError::InvalidAccountIndex // Obscure precompile error
    );
}
