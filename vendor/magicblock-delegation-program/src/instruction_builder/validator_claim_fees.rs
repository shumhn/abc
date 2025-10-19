use borsh::to_vec;
use solana_program::instruction::Instruction;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey};

use crate::args::ValidatorClaimFeesArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::{fees_vault_pda, validator_fees_vault_pda_from_validator};

/// Claim the accrued fees from the fees vault.
/// See [crate::processor::process_validator_claim_fees] for docs.
pub fn validator_claim_fees(validator: Pubkey, amount: Option<u64>) -> Instruction {
    let args = ValidatorClaimFeesArgs { amount };
    let fees_vault_pda = fees_vault_pda();
    let validator_fees_vault_pda = validator_fees_vault_pda_from_validator(&validator);
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(validator, true),
            AccountMeta::new(fees_vault_pda, false),
            AccountMeta::new(validator_fees_vault_pda, false),
        ],
        data: [
            DlpDiscriminator::ValidatorClaimFees.to_vec(),
            to_vec(&args).unwrap(),
        ]
        .concat(),
    }
}
