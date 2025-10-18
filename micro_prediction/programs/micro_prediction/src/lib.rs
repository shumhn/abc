use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use ephemeral_rollups_sdk::anchor::ephemeral;
use ephemeral_rollups_sdk::cpi::{commit_and_undelegate_accounts, commit_accounts, delegate_account, undelegate_account};
use ephemeral_rollups_sdk::cpi::DelegateAccounts;
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use ephemeral_rollups_sdk::consts::{DELEGATION_PROGRAM_ID, MAGIC_CONTEXT_ID, MAGIC_PROGRAM_ID};
use ephemeral_rollups_sdk::delegate_args::DelegateAccounts as DelegateAccountAddresses;
use pyth_sdk_solana::state::load_price_feed_from_account_info;

pub const GLOBAL_STATE_SEED: &[u8] = b"global-state";
pub const ROUND_STATE_SEED: &[u8] = b"round";
pub const ROUND_LEDGER_SEED: &[u8] = b"round-ledger";
pub const ROUND_ESCROW_SEED: &[u8] = b"round-escrow";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"vault-authority";
pub const MAGIC_DELEGATED_STATE_SEED: &[u8] = b"magic-delegated-state";
pub const MAGIC_DELEGATED_LEDGER_SEED: &[u8] = b"magic-delegated-ledger";
const PYTH_PRICE_STALENESS_THRESHOLD_SECS: i64 = 60;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DelegateRoundConfig {
    pub commit_frequency_ms: Option<u32>;
    pub validator: Option<Pubkey>;
}

declare_id!("3btqev6Y8xNxqwFxFKaDPihQyVZ1gs2DpBNsDukmHxNX");

#[ephemeral]
#[program]
pub mod micro_prediction {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        round_duration_secs: i64,
        max_predictions_per_round: u32,
        max_predictions_per_user: u8,
    ) -> Result<()> {
        require!(round_duration_secs > 0, ErrorCode::InvalidRoundDuration);
        require!(
            max_predictions_per_round > 0,
            ErrorCode::InvalidPredictionLimit
        );
        require!(
            max_predictions_per_user > 0,
            ErrorCode::InvalidPredictionLimit
        );

        let global_state = &mut ctx.accounts.global_state;
        global_state.authority = ctx.accounts.authority.key();
        global_state.token_mint = ctx.accounts.token_mint.key();
        global_state.round_duration_secs = round_duration_secs;
        global_state.max_predictions_per_round = max_predictions_per_round;
        global_state.max_predictions_per_user = max_predictions_per_user;
        global_state.global_state_bump = *ctx.bumps.get("global_state").unwrap();
        global_state.vault_authority_bump = *ctx.bumps.get("vault_authority").unwrap();
        global_state.round_counter = 0;
        Ok(())
    }

    pub fn initialize_round(ctx: Context<InitializeRound>, round_id: u64) -> Result<()> {
        let clock = Clock::get()?;
        let global_state = &mut ctx.accounts.global_state;
        require_keys_eq!(ctx.accounts.authority.key(), global_state.authority);
        require!(
            round_id > global_state.round_counter,
            ErrorCode::RoundAlreadyExists
        );

        let round_state = &mut ctx.accounts.round_state;
        round_state.round_id = round_id;
        round_state.start_ts = clock.unix_timestamp;
        round_state.end_ts = clock
            .unix_timestamp
            .checked_add(global_state.round_duration_secs)
            .ok_or(ErrorCode::MathOverflow)?;
        round_state.total_stake = 0;
        round_state.status = RoundStatus::Open;
        round_state.final_price = None;
        round_state.winners_count = 0;
        round_state.total_winner_stake = 0;
        round_state.delegation_status = DelegationStatus::NotDelegated;
        round_state.round_state_bump = *ctx.bumps.get("round_state").unwrap();
        round_state.reward_vault_bump = *ctx.bumps.get("round_escrow").unwrap();

        let ledger = &mut ctx.accounts.prediction_ledger;
        ledger.round_id = round_id;
        ledger.max_entries = global_state.max_predictions_per_round as usize;
        ledger.records = Vec::new();
        ledger.ledger_bump = *ctx.bumps.get("prediction_ledger").unwrap();

        global_state.round_counter = round_id;
        Ok(())
    }

    pub fn place_prediction(
        ctx: Context<PlacePrediction>,
        round_id: u64,
        predicted_price: u64,
        stake_amount: u64,
    ) -> Result<()> {
        require!(stake_amount > 0, ErrorCode::InvalidStakeAmount);

        let global_state = &ctx.accounts.global_state;
        let round_state = &mut ctx.accounts.round_state;
        require!(round_state.round_id == round_id, ErrorCode::RoundMismatch);
        require!(
            round_state.status == RoundStatus::Open,
            ErrorCode::RoundNotOpen
        );

        let clock = Clock::get()?;
        require!(
            clock.unix_timestamp <= round_state.end_ts,
            ErrorCode::RoundExpired
        );

        let ledger = &mut ctx.accounts.prediction_ledger;
        require!(ledger.round_id == round_id, ErrorCode::RoundMismatch);
        require!(
            ledger.records.len() < ledger.max_entries as usize,
            ErrorCode::RoundPredictionCapacityReached
        );

        let existing_predictions_for_user = ledger
            .records
            .iter()
            .filter(|record| record.user == ctx.accounts.player.key())
            .count();
        require!(
            existing_predictions_for_user < global_state.max_predictions_per_user as usize,
            ErrorCode::PredictionLimitExceeded
        );

        ledger.records.push(PredictionRecord {
            user: ctx.accounts.player.key(),
            predicted_price,
            stake: stake_amount,
            abs_diff: None,
            is_winner: false,
            claimed: false,
        });

        round_state.total_stake = round_state
            .total_stake
            .checked_add(stake_amount)
            .ok_or(ErrorCode::MathOverflow)?;

        let cpi_accounts = Transfer {
            from: ctx.accounts.player_token_account.to_account_info(),
            to: ctx.accounts.round_escrow.to_account_info(),
            authority: ctx.accounts.player.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, stake_amount)?;

        Ok(())
    }

    pub fn close_round(ctx: Context<CloseRound>, _final_price: Option<u64>) -> Result<()> {
    pub fn close_round(ctx: Context<CloseRound>, _final_price: Option<u64>) -> Result<()> {
        let global_state = &ctx.accounts.global_state;
        require_keys_eq!(ctx.accounts.authority.key(), global_state.authority);

        let clock = Clock::get()?;
        let round_state = &mut ctx.accounts.round_state;
        require!(
            round_state.status == RoundStatus::Open,
            ErrorCode::RoundNotOpen
        );

        let ledger = &mut ctx.accounts.prediction_ledger;
        require!(
            ledger.round_id == round_state.round_id,
            ErrorCode::RoundMismatch
        );
        require!(!ledger.records.is_empty(), ErrorCode::NoPredictionsPlaced);

        let final_price = load_and_validate_price(&ctx.accounts.pyth_price_feed, &clock)?;

        let mut min_diff: Option<u64> = None;
        let mut total_winner_stake: u64 = 0;

        for record in ledger.records.iter_mut() {
            let diff = if record.predicted_price >= final_price {
                record.predicted_price - final_price
            } else {
                final_price - record.predicted_price
            };
            record.abs_diff = Some(diff);
            if let Some(current_min) = min_diff {
                if diff < current_min {
                    min_diff = Some(diff);
                }
            } else {
                min_diff = Some(diff);
            }
        }

        let winning_diff = min_diff.ok_or(ErrorCode::NoPredictionsPlaced)?;

        for record in ledger.records.iter_mut() {
            if record.abs_diff == Some(winning_diff) {
                record.is_winner = true;
                total_winner_stake = total_winner_stake
                    .checked_add(record.stake)
                    .ok_or(ErrorCode::MathOverflow)?;
            }
        }

        require!(total_winner_stake > 0, ErrorCode::NoWinningPredictions);

        round_state.total_winner_stake = total_winner_stake;
        round_state.winners_count = ledger
            .records
            .iter()
            .filter(|record| record.is_winner)
            .count() as u32;
        round_state.status = RoundStatus::Settled;
        round_state.final_price = Some(final_price);

        Ok(())
    }

    pub fn claim_reward(ctx: Context<ClaimReward>, round_id: u64) -> Result<()> {
        let round_state = &mut ctx.accounts.round_state;
        require!(round_state.round_id == round_id, ErrorCode::RoundMismatch);
        require!(
            round_state.status == RoundStatus::Settled,
            ErrorCode::RoundNotSettled
        );

        let ledger = &mut ctx.accounts.prediction_ledger;
        require!(ledger.round_id == round_id, ErrorCode::RoundMismatch);

        let mut payout_total: u64 = 0;
        let mut updated_records: Vec<usize> = Vec::new();

        for (idx, record) in ledger.records.iter_mut().enumerate() {
            if record.user == ctx.accounts.player.key() && record.is_winner && !record.claimed {
                let amount = round_state
                    .total_stake
                    .checked_mul(record.stake)
                    .ok_or(ErrorCode::MathOverflow)?
                    .checked_div(round_state.total_winner_stake)
                    .ok_or(ErrorCode::MathOverflow)?;
                payout_total = payout_total
                    .checked_add(amount)
                    .ok_or(ErrorCode::MathOverflow)?;
                record.claimed = true;
                updated_records.push(idx);
            }
        }

        require!(payout_total > 0, ErrorCode::NothingToClaim);

        let seeds: &[&[u8]] = &[
            VAULT_AUTHORITY_SEED,
            &[ctx.accounts.global_state.vault_authority_bump],
        ];
        let signer = &[seeds];

        let cpi_accounts = Transfer {
            from: ctx.accounts.round_escrow.to_account_info(),
            to: ctx.accounts.player_token_account.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        );
        token::transfer(cpi_ctx, payout_total)?;

        // mark for clarity although already set
        for idx in updated_records {
            ledger.records[idx].claimed = true;
        }

        Ok(())
    }

    pub fn delegate_round(
        ctx: Context<DelegateRound>,
        round_id: u64,
        config: Option<DelegateRoundConfig>,
    ) -> Result<()> {
        let global_state = &ctx.accounts.global_state;
        require_keys_eq!(ctx.accounts.authority.key(), global_state.authority);

        let round_state = &mut ctx.accounts.round_state;
        require!(round_state.round_id == round_id, ErrorCode::RoundMismatch);
        require!(
            round_state.status == RoundStatus::Open,
            ErrorCode::RoundDelegationInvalidStatus
        );
        require!(
            round_state.delegation_status == DelegationStatus::NotDelegated,
            ErrorCode::DelegationAlreadyActive
        );

        require_keys_eq!(ctx.accounts.owner_program.key(), crate::id());
        require_keys_eq!(ctx.accounts.delegation_program.key(), DELEGATION_PROGRAM_ID);

        let round_addresses = DelegateAccountAddresses::new(
            ctx.accounts.round_state.key(),
            ctx.accounts.owner_program.key(),
        );
        let ledger_addresses = DelegateAccountAddresses::new(
            ctx.accounts.prediction_ledger.key(),
            ctx.accounts.owner_program.key(),
        );

        require_keys_eq!(
            ctx.accounts.round_delegation_buffer.key(),
            round_addresses.delegate_buffer
        );
        require_keys_eq!(
            ctx.accounts.round_delegation_record.key(),
            round_addresses.delegation_record
        );
        require_keys_eq!(
            ctx.accounts.round_delegation_metadata.key(),
            round_addresses.delegation_metadata
        );
        require_keys_eq!(
            ctx.accounts.ledger_delegation_buffer.key(),
            ledger_addresses.delegate_buffer
        );
        require_keys_eq!(
            ctx.accounts.ledger_delegation_record.key(),
            ledger_addresses.delegation_record
        );
        require_keys_eq!(
            ctx.accounts.ledger_delegation_metadata.key(),
            ledger_addresses.delegation_metadata
        );

        validate_delegate_accounts(
            &ctx.accounts.round_delegation_record.to_account_info(),
            &ctx.accounts.round_delegation_metadata.to_account_info(),
        )?;
        validate_delegate_accounts(
            &ctx.accounts.ledger_delegation_record.to_account_info(),
            &ctx.accounts.ledger_delegation_metadata.to_account_info(),
        )?;

        let delegate_config = build_delegate_config(config.clone());

        let round_id_bytes = round_id.to_le_bytes();
        let round_seeds: &[&[u8]] = &[ROUND_STATE_SEED, &round_id_bytes];
        delegate_account(
            DelegateAccounts {
                payer: &ctx.accounts.authority.to_account_info(),
                pda: &ctx.accounts.round_state.to_account_info(),
                owner_program: &ctx.accounts.owner_program.to_account_info(),
                buffer: &ctx.accounts.round_delegation_buffer.to_account_info(),
                delegation_record: &ctx.accounts.round_delegation_record.to_account_info(),
                delegation_metadata: &ctx.accounts.round_delegation_metadata.to_account_info(),
                delegation_program: &ctx.accounts.delegation_program.to_account_info(),
                system_program: &ctx.accounts.system_program.to_account_info(),
            },
            round_seeds,
            delegate_config,
        )?;

        let ledger_seeds: &[&[u8]] = &[ROUND_LEDGER_SEED, &round_id_bytes];
        let ledger_delegate_config = build_delegate_config(config);
        delegate_account(
            DelegateAccounts {
                payer: &ctx.accounts.authority.to_account_info(),
                pda: &ctx.accounts.prediction_ledger.to_account_info(),
                owner_program: &ctx.accounts.owner_program.to_account_info(),
                buffer: &ctx.accounts.ledger_delegation_buffer.to_account_info(),
                delegation_record: &ctx.accounts.ledger_delegation_record.to_account_info(),
                delegation_metadata: &ctx.accounts.ledger_delegation_metadata.to_account_info(),
                delegation_program: &ctx.accounts.delegation_program.to_account_info(),
                system_program: &ctx.accounts.system_program.to_account_info(),
            },
            ledger_seeds,
            ledger_delegate_config,
        )?;

        ctx.accounts.delegated_round_state.round_id = round_id;
        if let Some(bump) = ctx.bumps.get("delegated_round_state") {
            ctx.accounts.delegated_round_state.bump = *bump;
        }

        ctx.accounts.delegated_ledger_state.round_id = round_id;
        if let Some(bump) = ctx.bumps.get("delegated_ledger_state") {
            ctx.accounts.delegated_ledger_state.bump = *bump;
        }

        round_state.delegation_status = DelegationStatus::Delegated;

        Ok(())
    }

    pub fn commit_round(ctx: Context<CommitRound>, round_id: u64) -> Result<()> {
        let global_state = &ctx.accounts.global_state;
        require_keys_eq!(ctx.accounts.authority.key(), global_state.authority);

        let round_state = &ctx.accounts.round_state;
        require!(round_state.round_id == round_id, ErrorCode::RoundMismatch);
        require!(
            round_state.delegation_status == DelegationStatus::Delegated,
            ErrorCode::DelegationNotActive
        );
        require!(
            round_state.status == RoundStatus::Settled,
            ErrorCode::RoundDelegationInvalidStatus
        );

        require_keys_eq!(ctx.accounts.magic_context.key(), MAGIC_CONTEXT_ID);
        require_keys_eq!(ctx.accounts.magic_program.key(), MAGIC_PROGRAM_ID);

        commit_accounts(
            &ctx.accounts.authority.to_account_info(),
            vec![
                &ctx.accounts.round_state.to_account_info(),
                &ctx.accounts.prediction_ledger.to_account_info(),
            ],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program.to_account_info(),
        )?;

        Ok(())
    }

    pub fn commit_and_undelegate_round(
        ctx: Context<CommitAndUndelegateRound>,
        round_id: u64,
    ) -> Result<()> {
        let global_state = &ctx.accounts.global_state;
        require_keys_eq!(ctx.accounts.authority.key(), global_state.authority);

        let round_state = &mut ctx.accounts.round_state;
        require!(round_state.round_id == round_id, ErrorCode::RoundMismatch);
        require!(
            round_state.delegation_status == DelegationStatus::Delegated,
            ErrorCode::DelegationNotActive
        );
        require!(
            round_state.status == RoundStatus::Settled,
            ErrorCode::RoundDelegationInvalidStatus
        );

        require_keys_eq!(ctx.accounts.magic_context.key(), MAGIC_CONTEXT_ID);
        require_keys_eq!(ctx.accounts.magic_program.key(), MAGIC_PROGRAM_ID);

        commit_and_undelegate_accounts(
            &ctx.accounts.authority.to_account_info(),
            vec![
                &ctx.accounts.round_state.to_account_info(),
                &ctx.accounts.prediction_ledger.to_account_info(),
            ],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program.to_account_info(),
        )?;

        round_state.delegation_status = DelegationStatus::CommitScheduled;

        Ok(())
    }

    pub fn undelegate_round(ctx: Context<UndelegateRound>, round_id: u64) -> Result<()> {
        let global_state = &ctx.accounts.global_state;
        require_keys_eq!(ctx.accounts.authority.key(), global_state.authority);

        let round_state = &mut ctx.accounts.round_state;
        require!(round_state.round_id == round_id, ErrorCode::RoundMismatch);
        require!(
            matches!(
                round_state.delegation_status,
                DelegationStatus::Delegated | DelegationStatus::CommitScheduled
            ),
            ErrorCode::DelegationNotActive
        );

        require_keys_eq!(ctx.accounts.owner_program.key(), crate::id());
        require_keys_eq!(ctx.accounts.delegation_program.key(), DELEGATION_PROGRAM_ID);

        let round_addresses = DelegateAccountAddresses::new(
            ctx.accounts.round_state.key(),
            ctx.accounts.owner_program.key(),
        );
        let ledger_addresses = DelegateAccountAddresses::new(
            ctx.accounts.prediction_ledger.key(),
            ctx.accounts.owner_program.key(),
        );

        require_keys_eq!(
            ctx.accounts.round_delegation_buffer.key(),
            round_addresses.delegate_buffer
        );
        require_keys_eq!(
            ctx.accounts.round_delegation_record.key(),
            round_addresses.delegation_record
        );
        require_keys_eq!(
            ctx.accounts.round_delegation_metadata.key(),
            round_addresses.delegation_metadata
        );
        require_keys_eq!(
            ctx.accounts.ledger_delegation_buffer.key(),
            ledger_addresses.delegate_buffer
        );
        require_keys_eq!(
            ctx.accounts.ledger_delegation_record.key(),
            ledger_addresses.delegation_record
        );
        require_keys_eq!(
            ctx.accounts.ledger_delegation_metadata.key(),
            ledger_addresses.delegation_metadata
        );

        validate_delegate_accounts(
            &ctx.accounts.round_delegation_record,
            &ctx.accounts.round_delegation_metadata,
        )?;
        validate_delegate_accounts(
            &ctx.accounts.ledger_delegation_record,
            &ctx.accounts.ledger_delegation_metadata,
        )?;

        let round_id_bytes = round_id.to_le_bytes();
        undelegate_account(
            &ctx.accounts.round_state.to_account_info(),
            ctx.accounts.owner_program.key,
            &ctx.accounts.round_delegation_buffer,
            &ctx.accounts.authority.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            vec![ROUND_STATE_SEED.to_vec(), round_id_bytes.to_vec()],
        )?;

        let ledger_round_bytes = round_id.to_le_bytes();
        undelegate_account(
            &ctx.accounts.prediction_ledger.to_account_info(),
            ctx.accounts.owner_program.key,
            &ctx.accounts.ledger_delegation_buffer,
            &ctx.accounts.authority.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            vec![ROUND_LEDGER_SEED.to_vec(), ledger_round_bytes.to_vec()],
        )?;

        round_state.delegation_status = DelegationStatus::NotDelegated;

        Ok(())
    }
}

fn load_and_validate_price(price_feed_info: &AccountInfo, clock: &Clock) -> Result<u64> {
    let price_feed = load_price_feed_from_account_info(price_feed_info)
        .map_err(|_| error!(ErrorCode::PythPriceNotAvailable))?;
    let price = price_feed
        .get_price_no_older_than(clock.unix_timestamp, PYTH_PRICE_STALENESS_THRESHOLD_SECS)
        .map_err(|_| error!(ErrorCode::PythPriceStale))?;
    let decimal_exponent = price
        .expo
        .checked_neg()
        .ok_or(error!(ErrorCode::PythPriceScalingError))?;
    let scale = 10u128
        .checked_pow(decimal_exponent as u32)
        .ok_or(error!(ErrorCode::PythPriceScalingError))?;
    let price_scaled = (price.price as i128)
        .checked_abs()
        .ok_or(error!(ErrorCode::PythPriceScalingError))? as u128;
    let display_price = price_scaled
        .checked_div(scale)
        .ok_or(error!(ErrorCode::PythPriceScalingError))?;
    display_price
        .try_into()
        .map_err(|_| error!(ErrorCode::PythPriceScalingError))
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = authority,
        seeds = [GLOBAL_STATE_SEED],
        bump,
        space = GlobalState::SPACE,
    )]
    pub global_state: Account<'info, GlobalState>,
    /// CHECK: PDA authority, no data stored
    #[account(
        seeds = [VAULT_AUTHORITY_SEED],
        bump,
    )]
    pub vault_authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct InitializeRound<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut, seeds = [GLOBAL_STATE_SEED], bump = global_state.global_state_bump)]
    pub global_state: Account<'info, GlobalState>,
    #[account(
        init,
        payer = authority,
        seeds = [ROUND_STATE_SEED, &round_id.to_le_bytes()],
        bump,
        space = RoundState::SPACE,
    )]
    pub round_state: Account<'info, RoundState>,
    #[account(
        init,
        payer = authority,
        seeds = [ROUND_LEDGER_SEED, &round_id.to_le_bytes()],
        bump,
        space = PredictionLedger::space(global_state.max_predictions_per_round as usize),
    )]
    pub prediction_ledger: Account<'info, PredictionLedger>,
    #[account(
        init,
        payer = authority,
        seeds = [ROUND_ESCROW_SEED, &round_id.to_le_bytes()],
        bump,
        token::mint = global_state.token_mint,
        token::authority = vault_authority,
    )]
    pub round_escrow: Account<'info, TokenAccount>,
    /// CHECK: PDA authority for token escrow
    #[account(
        seeds = [VAULT_AUTHORITY_SEED],
        bump = global_state.vault_authority_bump,
    )]
    pub vault_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(round_id: u64, predicted_price: u64, stake_amount: u64)]
pub struct PlacePrediction<'info> {
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(seeds = [GLOBAL_STATE_SEED], bump = global_state.global_state_bump)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut, seeds = [ROUND_STATE_SEED, &round_id.to_le_bytes()], bump = round_state.round_state_bump)]
    pub round_state: Account<'info, RoundState>,
    #[account(mut, seeds = [ROUND_LEDGER_SEED, &round_id.to_le_bytes()], bump = prediction_ledger.ledger_bump)]
    pub prediction_ledger: Account<'info, PredictionLedger>,
    #[account(
        mut,
        seeds = [ROUND_ESCROW_SEED, &round_id.to_le_bytes()],
        bump = round_state.reward_vault_bump,
    )]
    pub round_escrow: Account<'info, TokenAccount>,
    #[account(mut, constraint = player_token_account.mint == global_state.token_mint)]
    pub player_token_account: Account<'info, TokenAccount>,
    /// CHECK: PDA authority for escrow account
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_state.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CloseRound<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut, seeds = [GLOBAL_STATE_SEED], bump = global_state.global_state_bump)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut, seeds = [ROUND_STATE_SEED, &round_state.round_id.to_le_bytes()], bump = round_state.round_state_bump)]
    pub round_state: Account<'info, RoundState>,
    #[account(mut, seeds = [ROUND_LEDGER_SEED, &round_state.round_id.to_le_bytes()], bump = prediction_ledger.ledger_bump)]
    pub prediction_ledger: Account<'info, PredictionLedger>,
    /// CHECK: Verified via Pyth SDK
    pub pyth_price_feed: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ClaimReward<'info> {
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(seeds = [GLOBAL_STATE_SEED], bump = global_state.global_state_bump)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut, seeds = [ROUND_STATE_SEED, &round_id.to_le_bytes()], bump = round_state.round_state_bump)]
    pub round_state: Account<'info, RoundState>,
    #[account(mut, seeds = [ROUND_LEDGER_SEED, &round_id.to_le_bytes()], bump = prediction_ledger.ledger_bump)]
    pub prediction_ledger: Account<'info, PredictionLedger>,
    #[account(
        mut,
        seeds = [ROUND_ESCROW_SEED, &round_id.to_le_bytes()],
        bump = round_state.reward_vault_bump,
    )]
    pub round_escrow: Account<'info, TokenAccount>,
    #[account(mut, constraint = player_token_account.mint == global_state.token_mint)]
    pub player_token_account: Account<'info, TokenAccount>,
    /// CHECK: PDA authority for escrow account
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_state.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct DelegateRound<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds = [GLOBAL_STATE_SEED], bump = global_state.global_state_bump)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut, seeds = [ROUND_STATE_SEED, &round_id.to_le_bytes()], bump = round_state.round_state_bump)]
    pub round_state: Account<'info, RoundState>,
    #[account(mut, seeds = [ROUND_LEDGER_SEED, &round_id.to_le_bytes()], bump = prediction_ledger.ledger_bump)]
    pub prediction_ledger: Account<'info, PredictionLedger>,
    #[account(
        init,
        payer = authority,
        seeds = [MAGIC_DELEGATED_STATE_SEED, &round_id.to_le_bytes()],
        bump,
        space = DelegatedRoundState::SPACE,
    )]
    pub delegated_round_state: Account<'info, DelegatedRoundState>,
    #[account(
        init,
        payer = authority,
        seeds = [MAGIC_DELEGATED_LEDGER_SEED, &round_id.to_le_bytes()],
        bump,
        space = DelegatedLedgerState::SPACE,
    )]
    pub delegated_ledger_state: Account<'info, DelegatedLedgerState>,
    /// CHECK: This is the program ID
    pub owner_program: AccountInfo<'info>,
    /// CHECK: Delegation program buffer PDA
    #[account(mut)]
    pub round_delegation_buffer: AccountInfo<'info>,
    /// CHECK: Delegation record PDA
    #[account(mut)]
    pub round_delegation_record: AccountInfo<'info>,
    /// CHECK: Delegation metadata PDA
    #[account(mut)]
    pub round_delegation_metadata: AccountInfo<'info>,
    /// CHECK: Delegation program buffer PDA for ledger
    #[account(mut)]
    pub ledger_delegation_buffer: AccountInfo<'info>,
    /// CHECK: Delegation record PDA for ledger
    #[account(mut)]
    pub ledger_delegation_record: AccountInfo<'info>,
    /// CHECK: Delegation metadata PDA for ledger
    #[account(mut)]
    pub ledger_delegation_metadata: AccountInfo<'info>,
    /// CHECK: Delegation program
    pub delegation_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct CommitRound<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds = [GLOBAL_STATE_SEED], bump = global_state.global_state_bump)]
    pub global_state: Account<'info, GlobalState>,
    #[account(seeds = [ROUND_STATE_SEED, &round_id.to_le_bytes()], bump = round_state.round_state_bump)]
    pub round_state: Account<'info, RoundState>,
    #[account(seeds = [ROUND_LEDGER_SEED, &round_id.to_le_bytes()], bump = prediction_ledger.ledger_bump)]
    pub prediction_ledger: Account<'info, PredictionLedger>,
    /// CHECK: Magic context PDA
    #[account(mut)]
    pub magic_context: AccountInfo<'info>,
    /// CHECK: Magic program
    pub magic_program: AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct CommitAndUndelegateRound<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds = [GLOBAL_STATE_SEED], bump = global_state.global_state_bump)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut, seeds = [ROUND_STATE_SEED, &round_id.to_le_bytes()], bump = round_state.round_state_bump)]
    pub round_state: Account<'info, RoundState>,
    #[account(mut, seeds = [ROUND_LEDGER_SEED, &round_id.to_le_bytes()], bump = prediction_ledger.ledger_bump)]
    pub prediction_ledger: Account<'info, PredictionLedger>,
    /// CHECK: Magic context PDA
    #[account(mut)]
    pub magic_context: AccountInfo<'info>,
    /// CHECK: Magic program
    pub magic_program: AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct UndelegateRound<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds = [GLOBAL_STATE_SEED], bump = global_state.global_state_bump)]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut, seeds = [ROUND_STATE_SEED, &round_id.to_le_bytes()], bump = round_state.round_state_bump)]
    pub round_state: Account<'info, RoundState>,
    #[account(mut, seeds = [ROUND_LEDGER_SEED, &round_id.to_le_bytes()], bump = prediction_ledger.ledger_bump)]
    pub prediction_ledger: Account<'info, PredictionLedger>,
    #[account(
        mut,
        close = authority,
        seeds = [MAGIC_DELEGATED_STATE_SEED, &round_id.to_le_bytes()],
        bump = delegated_round_state.bump,
    )]
    pub delegated_round_state: Account<'info, DelegatedRoundState>,
    #[account(
        mut,
        close = authority,
        seeds = [MAGIC_DELEGATED_LEDGER_SEED, &round_id.to_le_bytes()],
        bump = delegated_ledger_state.bump,
    )]
    pub delegated_ledger_state: Account<'info, DelegatedLedgerState>,
    /// CHECK: This is the program ID
    pub owner_program: AccountInfo<'info>,
    /// CHECK: Delegation program buffer PDA
    #[account(mut)]
    pub round_delegation_buffer: AccountInfo<'info>,
    /// CHECK: Delegation record PDA
    #[account(mut)]
    pub round_delegation_record: AccountInfo<'info>,
    /// CHECK: Delegation metadata PDA
    #[account(mut)]
    pub round_delegation_metadata: AccountInfo<'info>,
    /// CHECK: Delegation program buffer PDA for ledger
    #[account(mut)]
    pub ledger_delegation_buffer: AccountInfo<'info>,
    /// CHECK: Delegation record PDA for ledger
    #[account(mut)]
    pub ledger_delegation_record: AccountInfo<'info>,
    /// CHECK: Delegation metadata PDA for ledger
    #[account(mut)]
    pub ledger_delegation_metadata: AccountInfo<'info>,
    /// CHECK: Delegation program
    pub delegation_program: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

fn build_delegate_config(config: Option<DelegateRoundConfig>) -> DelegateConfig {
    match config {
        Some(custom) => DelegateConfig {
            commit_frequency_ms: custom
                .commit_frequency_ms
                .unwrap_or_else(|| DelegateConfig::default().commit_frequency_ms),
            validator: custom.validator,
        },
        None => DelegateConfig::default(),
    }
}

fn validate_delegate_accounts(record: &AccountInfo, metadata: &AccountInfo) -> Result<()> {
    require_keys_eq!(record.owner, &DELEGATION_PROGRAM_ID);
    require_keys_eq!(metadata.owner, &DELEGATION_PROGRAM_ID);
    Ok(())
}

#[account]
pub struct GlobalState {
    pub authority: Pubkey,
    pub token_mint: Pubkey,
    pub round_duration_secs: i64,
    pub max_predictions_per_round: u32,
    pub max_predictions_per_user: u8,
    pub global_state_bump: u8,
    pub vault_authority_bump: u8,
    pub round_counter: u64,
}

impl GlobalState {
    pub const SPACE: usize = 8  // discriminator
        + 32                   // authority
        + 32                   // token_mint
        + 8                    // round_duration_secs
        + 4                    // max_predictions_per_round
        + 1                    // max_predictions_per_user
        + 1                    // global_state_bump
        + 1                    // vault_authority_bump
        + 8; // round_counter
}

#[account]
pub struct RoundState {
    pub round_id: u64,
    pub start_ts: i64,
    pub end_ts: i64,
    pub total_stake: u64,
    pub status: RoundStatus,
    pub final_price: Option<u64>,
    pub winners_count: u32,
    pub total_winner_stake: u64,
    pub delegation_status: DelegationStatus,
    pub round_state_bump: u8,
    pub reward_vault_bump: u8,
}

impl RoundState {
    pub const SPACE: usize = 8   // discriminator
        + 8                      // round_id
        + 8                      // start_ts
        + 8                      // end_ts
        + 8                      // total_stake
        + 1                      // status
        + 1 + 8                  // option final_price (anchor stores bool + value)
        + 4                      // winners_count
        + 8                      // total_winner_stake
        + 1                      // delegation_status
        + 1                      // round_state_bump
        + 1; // reward_vault_bump
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum RoundStatus {
    Open,
    Settled,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum DelegationStatus {
    NotDelegated,
    Delegated,
    CommitScheduled,
}

#[account]
pub struct PredictionLedger {
    pub round_id: u64,
    pub max_entries: u32,
    pub records: Vec<PredictionRecord>,
    pub ledger_bump: u8,
}

impl PredictionLedger {
    pub fn space(max_entries: usize) -> usize {
        8  // discriminator
            + 8  // round_id
            + 4  // max_entries
            + 4  // vec length prefix
            + max_entries * PredictionRecord::SPACE
            + 1 // ledger_bump
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PredictionRecord {
    pub user: Pubkey,
    pub predicted_price: u64,
    pub stake: u64,
    pub abs_diff: Option<u64>,
    pub is_winner: bool,
    pub claimed: bool,
}

impl PredictionRecord {
    pub const SPACE: usize = 32   // user
        + 8                      // predicted_price
        + 8                      // stake
        + (1 + 8)                // Option<u64>
        + 1                      // is_winner
        + 1; // claimed
}

#[error_code]
pub enum ErrorCode {
    #[msg("Round duration must be positive")]
    InvalidRoundDuration,
    #[msg("Prediction limits must be positive")]
    InvalidPredictionLimit,
    #[msg("Round already exists or identifier reused")]
    RoundAlreadyExists,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Round mismatch between accounts")]
    RoundMismatch,
    #[msg("Round is not open")]
    RoundNotOpen,
    #[msg("Round has expired")]
    RoundExpired,
    #[msg("Prediction limit per user reached")]
    PredictionLimitExceeded,
    #[msg("Round prediction capacity reached")]
    RoundPredictionCapacityReached,
    #[msg("Stake amount must be positive")]
    InvalidStakeAmount,
    #[msg("No predictions placed for this round")]
    NoPredictionsPlaced,
    #[msg("No winning predictions were found")]
    NoWinningPredictions,
    #[msg("Round has not been settled")]
    RoundNotSettled,
    #[msg("Nothing to claim")]
    NothingToClaim,
    #[msg("Pyth price not available")]
    PythPriceNotAvailable,
    #[msg("Pyth price is stale")]
    PythPriceStale,
    #[msg("Failed to scale Pyth price")]
    PythPriceScalingError,
    #[msg("Delegate accounts already active for this round")]
    DelegationAlreadyActive,
    #[msg("No delegation active for this round")]
    DelegationNotActive,
    #[msg("Round status prevents delegation")]
    RoundDelegationInvalidStatus,
}

#[account]
pub struct DelegatedRoundState {
    pub round_id: u64,
    pub bump: u8,
}

impl DelegatedRoundState {
    pub const SPACE: usize = 8 + 8 + 1;
}

#[account]
pub struct DelegatedLedgerState {
    pub round_id: u64,
    pub bump: u8,
}

impl DelegatedLedgerState {
    pub const SPACE: usize = 8 + 8 + 1;
}
