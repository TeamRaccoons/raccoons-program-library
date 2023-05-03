use bytemuck::bytes_of;
use signed_data::Ed25519SignatureOffsets;
use solana_sdk::{
    ed25519_instruction::{DATA_START, PUBKEY_SERIALIZED_SIZE, SIGNATURE_SERIALIZED_SIZE},
    instruction::Instruction,
};

/// This is to verify a message from ix data of another instruction
/// The solana-sdk new_ed25519_instruction does not allow this
pub fn new_ed25519_instruction_without_payload(
    message: &[u8],
    ix_index: u16,
    offset: u16,
) -> Instruction {
    let mut instruction_data = Vec::with_capacity(
        DATA_START
            .saturating_add(SIGNATURE_SERIALIZED_SIZE)
            .saturating_add(PUBKEY_SERIALIZED_SIZE)
            .saturating_add(message.len()),
    );

    let num_signatures: u8 = 1;
    let signature_offset = offset as usize;
    let public_key_offset = signature_offset.saturating_add(SIGNATURE_SERIALIZED_SIZE);
    let message_data_offset = public_key_offset.saturating_add(PUBKEY_SERIALIZED_SIZE);

    // add padding byte so that offset structure is aligned
    instruction_data.extend_from_slice(bytes_of(&[num_signatures, 0]));

    let offsets = Ed25519SignatureOffsets {
        signature_offset: signature_offset as u16,
        signature_instruction_index: ix_index,
        public_key_offset: public_key_offset as u16,
        public_key_instruction_index: ix_index,
        message_data_offset: message_data_offset as u16,
        message_data_size: message.len() as u16,
        message_instruction_index: ix_index,
    };

    instruction_data.extend_from_slice(bytes_of(&offsets));

    Instruction {
        program_id: solana_sdk::ed25519_program::id(),
        accounts: vec![],
        data: instruction_data,
    }
}
