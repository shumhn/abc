use crate::args::ValidatorClaimFeesArgs;
use crate::consts::PROTOCOL_FEES_PERCENTAGE;
use crate::error::DlpError;
use crate::processor::utils::loaders::{
    load_initialized_protocol_fees_vault, load_initialized_validator_fees_vault, load_signer,
};
use borsh::BorshDeserialize;
use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::rent::Rent;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

/// Process validator request to claim fees from the fees vault
///
/// Accounts:
///
/// 0: `[signer]`   the validator account.
/// 1: `[writable]` the fees vault PDA.
/// 2: `[writable]` the validator fees vault PDA.
///
/// Requirements:
///
/// - protocol fees vault is initialized
/// - validator fees vault is initialized
/// - validators fees vault needs to hold enough lamports to claim
///
/// 1. Transfer lamports from validator fees_vault PDA to the validator authority
pub fn process_validator_claim_fees(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = ValidatorClaimFeesArgs::try_from_slice(data)?;

    // Load Accounts
    let [validator, fees_vault, validator_fees_vault] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(validator, "validator")?;
    load_initialized_protocol_fees_vault(fees_vault, true)?;
    load_initialized_validator_fees_vault(validator, validator_fees_vault, true)?;

    // Calculate the amount to transfer
    let min_rent = Rent::default().minimum_balance(8);
    let amount = args
        .amount
        .unwrap_or(validator_fees_vault.lamports() - min_rent);

    // Ensure vault has enough lamports
    if validator_fees_vault.lamports() - min_rent < amount {
        msg!(
            "Vault ({}) has insufficient funds: {} < {}",
            validator_fees_vault.key,
            validator_fees_vault.lamports() - min_rent,
            amount
        );
        return Err(ProgramError::InsufficientFunds);
    }

    // Calculate fees and remaining amount
    let protocol_fees = (amount * u64::from(PROTOCOL_FEES_PERCENTAGE)) / 100;
    let remaining_amount = amount.saturating_sub(protocol_fees);

    // Transfer fees to fees_vault
    **fees_vault.try_borrow_mut_lamports()? = fees_vault
        .lamports()
        .checked_add(protocol_fees)
        .ok_or(DlpError::Overflow)?;

    // Transfer remaining amount from validator_fees_vault to validator
    **validator_fees_vault.try_borrow_mut_lamports()? = validator_fees_vault
        .lamports()
        .checked_sub(amount)
        .ok_or(ProgramError::InsufficientFunds)?;

    **validator.try_borrow_mut_lamports()? = validator
        .lamports()
        .checked_add(remaining_amount)
        .ok_or(DlpError::Overflow)?;

    Ok(())
}
