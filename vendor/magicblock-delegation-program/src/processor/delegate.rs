use borsh::BorshDeserialize;
use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::sysvar::Sysvar;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_program,
};

use crate::args::DelegateArgs;
use crate::consts::DEFAULT_VALIDATOR_IDENTITY;
use crate::processor::utils::curve::is_on_curve;
use crate::processor::utils::loaders::{
    load_owned_pda, load_pda, load_program, load_signer, load_uninitialized_pda,
};
use crate::processor::utils::pda::create_pda;
use crate::state::{DelegationMetadata, DelegationRecord};
use crate::{
    delegate_buffer_seeds_from_delegated_account, delegation_metadata_seeds_from_delegated_account,
    delegation_record_seeds_from_delegated_account,
};

/// Delegates an account
///
/// Accounts:
/// 0: `[signer]`   the account paying for the transaction
/// 1: `[signer]`   the account to delegate
/// 2: `[]`         the owner of the account to delegate
/// 3: `[writable]` the buffer account we use to temporarily store the account data
///                 during owner change
/// 4: `[writable]` the delegation record account
/// 5: `[writable]` the delegation metadata account
/// 6: `[]`         the system program
///
/// Requirements:
///
/// - delegation buffer is initialized
/// - delegation record is uninitialized
/// - delegation metadata is uninitialized
///
/// Steps:
/// 1. Checks that the account is owned by the delegation program, that the buffer is initialized and derived correctly from the PDA
///  - Also checks that the delegated_account is a signer (enforcing that the instruction is being called from CPI) & other constraints
/// 2. Copies the data from the buffer into the original account
/// 3. Creates a Delegation Record to store useful information about the delegation event
/// 4. Creates a Delegated Account Seeds to store the seeds used to derive the delegate account. Needed for undelegation.
///
/// Usage:
///
/// This instruction is meant to be called via CPI with the owning program signing for the
/// delegated account.
pub fn process_delegate(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [payer, delegated_account, owner_program, delegate_buffer_account, delegation_record_account, delegation_metadata_account, system_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let args = DelegateArgs::try_from_slice(data)?;

    load_owned_pda(delegated_account, &crate::id(), "delegated account")?;
    load_program(system_program, system_program::id(), "system program")?;

    msg!("Delegating: {}", delegated_account.key);

    // Validate seeds if the delegate account is not on curve, i.e. is a PDA
    // If the owner is the system program, we check if the account is derived from the delegation program,
    // allowing delegation of escrow accounts
    if !is_on_curve(delegated_account.key) {
        let seeds_to_validate: Vec<&[u8]> = args.seeds.iter().map(|v| v.as_slice()).collect();
        let program_id = if owner_program.key.eq(&system_program::id()) {
            crate::id()
        } else {
            *owner_program.key
        };
        let (derived_pda, _) =
            Pubkey::find_program_address(seeds_to_validate.as_ref(), &program_id);

        if derived_pda.ne(delegated_account.key) {
            msg!(
                "Expected delegated PDA to be {}, but got {}",
                derived_pda,
                delegated_account.key
            );
            return Err(ProgramError::InvalidSeeds);
        }
    }

    // Check that the buffer PDA is initialized and derived correctly from the PDA
    load_pda(
        delegate_buffer_account,
        delegate_buffer_seeds_from_delegated_account!(delegated_account.key),
        owner_program.key,
        true,
        "delegate buffer",
    )?;

    // Check that the delegation record PDA is uninitialized
    let delegation_record_bump = load_uninitialized_pda(
        delegation_record_account,
        delegation_record_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
        true,
        "delegation record",
    )?;

    // Check that the delegation metadata PDA is uninitialized
    let delegation_metadata_bump = load_uninitialized_pda(
        delegation_metadata_account,
        delegation_metadata_seeds_from_delegated_account!(delegated_account.key),
        &crate::id(),
        true,
        "delegation metadata",
    )?;

    // Check that payer and delegated_account are signers, this ensures the instruction is being called from CPI
    load_signer(payer, "payer")?;
    load_signer(delegated_account, "delegated account")?;

    // Initialize the delegation record PDA
    create_pda(
        delegation_record_account,
        &crate::id(),
        DelegationRecord::size_with_discriminator(),
        delegation_record_seeds_from_delegated_account!(delegated_account.key),
        delegation_record_bump,
        system_program,
        payer,
    )?;

    // Initialize the delegation record
    let delegation_record = DelegationRecord {
        owner: *owner_program.key,
        authority: args.validator.unwrap_or(DEFAULT_VALIDATOR_IDENTITY),
        commit_frequency_ms: args.commit_frequency_ms as u64,
        delegation_slot: solana_program::clock::Clock::get()?.slot,
        lamports: delegated_account.lamports(),
    };
    let mut delegation_record_data = delegation_record_account.try_borrow_mut_data()?;
    delegation_record.to_bytes_with_discriminator(&mut delegation_record_data)?;

    // Initialize the account seeds PDA
    let mut delegation_metadata_bytes = vec![];
    let delegation_metadata = DelegationMetadata {
        seeds: args.seeds,
        last_update_nonce: 0,
        is_undelegatable: false,
        rent_payer: *payer.key,
    };
    delegation_metadata.to_bytes_with_discriminator(&mut delegation_metadata_bytes)?;

    // Initialize the delegation metadata PDA
    create_pda(
        delegation_metadata_account,
        &crate::id(),
        delegation_metadata_bytes.len(),
        delegation_metadata_seeds_from_delegated_account!(delegated_account.key),
        delegation_metadata_bump,
        system_program,
        payer,
    )?;

    // Copy the seeds to the delegated metadata PDA
    let mut delegation_metadata_data = delegation_metadata_account.try_borrow_mut_data()?;
    delegation_metadata_data.copy_from_slice(&delegation_metadata_bytes);

    // Copy the data from the buffer into the original account
    if !delegate_buffer_account.data_is_empty() {
        let mut delegated_data = delegated_account.try_borrow_mut_data()?;
        let delegate_buffer_data = delegate_buffer_account.try_borrow_data()?;
        (*delegated_data).copy_from_slice(&delegate_buffer_data);
    }

    Ok(())
}
