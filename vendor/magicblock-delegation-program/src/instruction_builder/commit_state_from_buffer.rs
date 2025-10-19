use borsh::to_vec;
use solana_program::instruction::Instruction;
use solana_program::system_program;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::CommitStateFromBufferArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::{
    commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    program_config_from_program_id, validator_fees_vault_pda_from_validator,
};

/// Builds a commit state from buffer instruction.
/// See [crate::processor::process_commit_state_from_buffer] for docs.
pub fn commit_state_from_buffer(
    validator: Pubkey,
    delegated_account: Pubkey,
    delegated_account_owner: Pubkey,
    commit_state_buffer: Pubkey,
    commit_args: CommitStateFromBufferArgs,
) -> Instruction {
    let commit_args = to_vec(&commit_args).unwrap();
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&delegated_account);
    let commit_state_pda = commit_state_pda_from_delegated_account(&delegated_account);
    let commit_record_pda = commit_record_pda_from_delegated_account(&delegated_account);
    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let program_config_pda = program_config_from_program_id(&delegated_account_owner);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(validator, true),
            AccountMeta::new_readonly(delegated_account, false),
            AccountMeta::new(commit_state_pda, false),
            AccountMeta::new(commit_record_pda, false),
            AccountMeta::new_readonly(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new_readonly(commit_state_buffer, false),
            AccountMeta::new_readonly(validator_fees_vault_pda, false),
            AccountMeta::new_readonly(program_config_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: [
            DlpDiscriminator::CommitStateFromBuffer.to_vec(),
            commit_args,
        ]
        .concat(),
    }
}
