use anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas};
use libsecp256k1::PublicKey;
use signed_ix;
use solana_program::{
    clock::Clock,
    instruction::{AccountMeta, InstructionError},
    sysvar,
};
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction,
    pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::{Transaction, TransactionError},
};

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
async fn test_signed_ix() {
    let program_id = signed_ix::ID;
    let mut program_test = ProgramTest::new("signed_ix", program_id, None);
    program_test.prefer_bpf(true);

    const NOOP_PROGRAM_ID: Pubkey = pubkey!("Noop111111111111111111111111111111111111112");
    program_test.add_program("noop", NOOP_PROGRAM_ID, None);

    let mut context = program_test.start_with_context().await;

    let mut rng = rand::thread_rng();
    let secp256k1_secret_key = libsecp256k1::SecretKey::random(&mut rng);
    let secp256k1_public_key = PublicKey::from_secret_key(&secp256k1_secret_key);
    let mut public_key_bytes = [0; 64];
    public_key_bytes.copy_from_slice(&secp256k1_public_key.serialize()[1..]);

    // Let's imagine a whattodo service which when called returns an expiry_at, an Instruction and
    // a signature from a known public key authority, for the serialized request + expiry_at + ix
    //
    // A program "signed-ix", allow those service ix to be executed at the top level, right before verification
    // trusting only the off-chain service, without having the off-chain service ever knowing about this program
    let wallet = Keypair::new();
    let payer = context.payer.pubkey();
    let request = "https://whattodo.so/ix";

    // First the verifying
    let config = Pubkey::find_program_address(&[b"config"], &signed_ix::ID).0;
    process_transaction(
        &mut context,
        &[Instruction {
            program_id: signed_ix::ID,
            accounts: signed_ix::accounts::Initialize {
                config,
                payer,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: signed_ix::instruction::Initialize {
                authority: public_key_bytes,
            }
            .data(),
        }],
        &[],
    )
    .await
    .unwrap();

    let expire_at = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .unwrap()
        .unix_timestamp
        + 100;
    let ix = Instruction {
        program_id: NOOP_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(wallet.pubkey(), true),
            AccountMeta::new_readonly(Pubkey::new_unique(), false),
            AccountMeta::new(Pubkey::new_unique(), false),
            AccountMeta::new_readonly(Pubkey::new_unique(), false),
            AccountMeta::new(Pubkey::new_unique(), false),
            AccountMeta::new_readonly(Pubkey::new_unique(), false),
            AccountMeta::new(Pubkey::new_unique(), false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
    };

    let message_hash = signed_ix::compute_message_hash(request, expire_at, &ix);
    let secp_message = libsecp256k1::Message::parse(&message_hash.to_bytes());
    let (signature, recovery_id) = libsecp256k1::sign(&secp_message, &secp256k1_secret_key);

    process_transaction(
        &mut context,
        &[
            ix.clone(),
            Instruction {
                program_id: signed_ix::ID,
                accounts: signed_ix::accounts::VerifySignedIx {
                    config,
                    instructions_sysvar: sysvar::instructions::ID,
                }
                .to_account_metas(None),
                data: signed_ix::instruction::VerifySignedIx {
                    signature: signature.serialize(),
                    recovery_id: recovery_id.serialize(),
                    request: request.into(),
                    expire_at,
                }
                .data(),
            },
        ],
        &[&wallet],
    )
    .await
    .unwrap();

    // Malicious actor tries to submit a modified ix
    let mut tampered_ix = ix;
    tampered_ix.accounts[1].pubkey = Pubkey::new_unique(); // changes 1 pubkey
    let result = process_transaction(
        &mut context,
        &[
            tampered_ix,
            Instruction {
                program_id: signed_ix::ID,
                accounts: signed_ix::accounts::VerifySignedIx {
                    config,
                    instructions_sysvar: sysvar::instructions::ID,
                }
                .to_account_metas(None),
                data: signed_ix::instruction::VerifySignedIx {
                    signature: signature.serialize(),
                    recovery_id: recovery_id.serialize(),
                    request: request.into(),
                    expire_at,
                }
                .data(),
            },
        ],
        &[&wallet],
    )
    .await;
    assert_eq!(
        result.unwrap_err().unwrap(),
        TransactionError::InstructionError(1, InstructionError::Custom(6001))
    );
}
