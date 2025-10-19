use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

// Custom getrandom implementation for Solana BPF
#[cfg(all(target_os = "solana", not(feature = "std")))]
#[no_mangle]
pub extern "C" fn getrandom(buf: *mut u8, len: usize) -> i32 {
    // Fill buffer with zeros (deterministic behavior for Solana on-chain)
    unsafe {
        core::ptr::write_bytes(buf, 0, len);
    }
    0 // Success
}

pub const CONFIG_SEED: &[u8] = b"config";
pub const ROUND_SEED: &[u8] = b"round";
pub const ESCROW_SEED: &[u8] = b"escrow";
pub const PREDICTION_SEED: &[u8] = b"prediction";

declare_id!("3btqev6Y8xNxqwFxFKaDPihQyVZ1gs2DpBNsDukmHxNX");

#[program]
pub mod micro_prediction {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        settlement_authority: Pubkey,
        fee_bps: u16,
    ) -> Result<()> {
        require!(fee_bps <= 10_000, ErrorCode::InvalidFeeBps);

        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.settlement_authority = settlement_authority;
        config.token_mint = ctx.accounts.token_mint.key();
        config.fee_treasury = ctx.accounts.fee_treasury.key();
        config.fee_bps = fee_bps;
        config.bump = ctx.bumps.config;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn initialize_round(
        ctx: Context<InitializeRound>,
        round_id: u64,
        start_ts: i64,
        end_ts: i64,
        pyth_price_account: Pubkey,
    ) -> Result<()> {
        require!(start_ts < end_ts, ErrorCode::InvalidRoundWindow);
        let clock = Clock::get()?;
        require!(start_ts >= clock.unix_timestamp, ErrorCode::RoundAlreadyActive);

        let round = &mut ctx.accounts.round;
        round.round_id = round_id;
        round.start_ts = start_ts;
        round.end_ts = end_ts;
        round.status = RoundStatus::Open as u8;
        round.token_mint = ctx.accounts.config.token_mint;
        round.escrow_vault = ctx.accounts.escrow_vault.key();
        round.total_stake = 0;
        round.total_paid = 0;
        round.final_price = None;
        round.settlement_timestamp = None;
        round.pyth_price_account = pyth_price_account;
        round.arcium_comp_id = None;
        round.result_commitment = None;
        round.bump = ctx.bumps.round;
        round.escrow_bump = ctx.bumps.escrow_vault;

        Ok(())
    }

    pub fn submit_prediction(
        ctx: Context<SubmitPrediction>,
        commitment: [u8; 32],
        window_index: u8,
        stake: u64,
        prediction_index: u16,
    ) -> Result<()> {
        require!(stake > 0, ErrorCode::InvalidStakeAmount);

        let clock = Clock::get()?;
        let round = &mut ctx.accounts.round;
        require!(round.status == RoundStatus::Open as u8, ErrorCode::RoundNotOpen);
        require!(clock.unix_timestamp >= round.start_ts, ErrorCode::RoundNotStarted);
        require!(clock.unix_timestamp <= round.end_ts, ErrorCode::RoundClosed);

        let config = &ctx.accounts.config;
        require_keys_eq!(config.token_mint, ctx.accounts.user_token_account.mint);
        require_keys_eq!(round.token_mint, config.token_mint);

        let seeds = [ROUND_SEED, &round.round_id.to_le_bytes(), &[round.bump]];
        let signer_seeds = [&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.escrow_vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
        );
        token::transfer(cpi_ctx, stake)?;

        round.total_stake = round
            .total_stake
            .checked_add(stake)
            .ok_or(ErrorCode::NumericalOverflow)?;

        let prediction = &mut ctx.accounts.prediction;
        prediction.round = round.key();
        prediction.owner = ctx.accounts.user.key();
        prediction.token_mint = round.token_mint;
        prediction.commitment = commitment;
        prediction.stake = stake;
        prediction.window_index = window_index;
        prediction.status = PredictionStatus::Submitted as u8;
        prediction.prediction_index = prediction_index;
        prediction.bump = ctx.bumps.prediction;

        Ok(())
    }

    pub fn cancel_prediction(ctx: Context<CancelPrediction>) -> Result<()> {
        let clock = Clock::get()?;
        let round = &mut ctx.accounts.round;
        require!(round.status == RoundStatus::Open as u8, ErrorCode::RoundNotOpen);
        require!(clock.unix_timestamp <= round.end_ts, ErrorCode::RoundClosed);

        let prediction = &mut ctx.accounts.prediction;
        require_keys_eq!(prediction.owner, ctx.accounts.user.key(), ErrorCode::Unauthorized);
        require!(
            prediction.status == PredictionStatus::Submitted as u8,
            ErrorCode::PredictionFinalized
        );

        let seeds = [ROUND_SEED, &round.round_id.to_le_bytes(), &[round.bump]];
        let signer_seeds = [&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.escrow_vault.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: round.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            &signer_seeds,
        );
        token::transfer(cpi_ctx, prediction.stake)?;

        round.total_stake = round
            .total_stake
            .checked_sub(prediction.stake)
            .ok_or(ErrorCode::NumericalOverflow)?;

        prediction.status = PredictionStatus::Cancelled as u8;
        prediction.stake = 0;

        Ok(())
    }

    pub fn begin_resolution(
        ctx: Context<BeginResolution>,
        result_commitment: Option<[u8; 32]>,
        arcium_comp_id: Option<Pubkey>,
    ) -> Result<()> {
        let round = &mut ctx.accounts.round;
        require!(round.status == RoundStatus::Open as u8, ErrorCode::RoundNotOpen);
        round.status = RoundStatus::Resolving as u8;
        round.result_commitment = result_commitment;
        round.arcium_comp_id = arcium_comp_id;
        Ok(())
    }

    pub fn settle_prediction(
        ctx: Context<SettlePrediction>,
        payout: u64,
        commitment: [u8; 32],
    ) -> Result<()> {
        let round = &mut ctx.accounts.round;
        let prediction = &mut ctx.accounts.prediction;
        
        require!(round.status == RoundStatus::Resolving as u8, ErrorCode::RoundNotResolving);
        require_keys_eq!(prediction.round, round.key(), ErrorCode::RoundMismatch);
        require!(
            prediction.status == PredictionStatus::Submitted as u8,
            ErrorCode::PredictionFinalized
        );
        require!(
            prediction.commitment == commitment,
            ErrorCode::CommitmentMismatch
        );

        // Verify sufficient funds
        let available = round
            .total_stake
            .checked_sub(round.total_paid)
            .ok_or(ErrorCode::NumericalOverflow)?;
        require!(payout <= available, ErrorCode::InsufficientEscrow);

        // Transfer payout
        if payout > 0 {
            let seeds = [ROUND_SEED, &round.round_id.to_le_bytes(), &[round.bump]];
            let signer_seeds = [&seeds[..]];
            let cpi_accounts = Transfer {
                from: ctx.accounts.escrow_vault.to_account_info(),
                to: ctx.accounts.recipient_token_account.to_account_info(),
                authority: round.to_account_info(),
            };
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                &signer_seeds,
            );
            token::transfer(cpi_ctx, payout)?;
        }

        // Update state
        round.total_paid = round
            .total_paid
            .checked_add(payout)
            .ok_or(ErrorCode::NumericalOverflow)?;
        prediction.status = PredictionStatus::Settled as u8;
        prediction.exit(&crate::ID)?;

        Ok(())
    }

    pub fn refund_prediction(ctx: Context<RefundPrediction>) -> Result<()> {
        let round = &ctx.accounts.round;
        let prediction = &mut ctx.accounts.prediction;
        
        require!(
            round.status == RoundStatus::Refunded as u8 || round.status == RoundStatus::Open as u8,
            ErrorCode::InvalidRoundState
        );
        require!(
            prediction.status == PredictionStatus::Submitted as u8
                || prediction.status == PredictionStatus::Cancelled as u8,
            ErrorCode::PredictionFinalized
        );
        require_keys_eq!(prediction.round, round.key(), ErrorCode::RoundMismatch);

        let amount = prediction.stake;
        if amount > 0 {
            let seeds = [ROUND_SEED, &round.round_id.to_le_bytes(), &[round.bump]];
            let signer_seeds = [&seeds[..]];
            let cpi_accounts = Transfer {
                from: ctx.accounts.escrow_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: round.to_account_info(),
            };
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                &signer_seeds,
            );
            token::transfer(cpi_ctx, amount)?;
        }

        prediction.status = PredictionStatus::Refunded as u8;
        prediction.stake = 0;
        prediction.exit(&crate::ID)?;

        Ok(())
    }

    pub fn finalize_round(ctx: Context<FinalizeRound>, final_price: i64, timestamp: i64) -> Result<()> {
        let round = &mut ctx.accounts.round;
        require!(round.status == RoundStatus::Resolving as u8, ErrorCode::RoundNotResolving);
        
        round.status = RoundStatus::Finalized as u8;
        round.final_price = Some(final_price);
        round.settlement_timestamp = Some(timestamp);

        Ok(())
    }

    pub fn mark_round_refunded(ctx: Context<MarkRoundRefunded>) -> Result<()> {
        let round = &mut ctx.accounts.round;
        require!(round.status != RoundStatus::Finalized as u8, ErrorCode::RoundAlreadySettled);
        round.status = RoundStatus::Refunded as u8;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut)]
    pub fee_treasury: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = authority,
        seeds = [CONFIG_SEED],
        bump,
        space = Config::SPACE,
    )]
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(round_id: u64, start_ts: i64, end_ts: i64, pyth_price_account: Pubkey)]
pub struct InitializeRound<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority @ ErrorCode::Unauthorized,
    )]
    pub config: Account<'info, Config>,
    #[account(
        init,
        payer = authority,
        seeds = [ROUND_SEED, &round_id.to_le_bytes()],
        bump,
        space = Round::SPACE,
    )]
    pub round: Account<'info, Round>,
    #[account(constraint = token_mint.key() == config.token_mint)]
    pub token_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = authority,
        seeds = [ESCROW_SEED, &round_id.to_le_bytes()],
        bump,
        token::mint = token_mint,
        token::authority = round,
    )]
    pub escrow_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(round_id: u64, prediction_index: u16)]
pub struct SubmitPrediction<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, &round_id.to_le_bytes()],
        bump = round.bump,
    )]
    pub round: Account<'info, Round>,
    #[account(
        init,
        payer = user,
        seeds = [
            PREDICTION_SEED,
            &round_id.to_le_bytes(),
            user.key().as_ref(),
            &prediction_index.to_le_bytes(),
        ],
        bump,
        space = Prediction::SPACE,
    )]
    pub prediction: Account<'info, Prediction>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [ESCROW_SEED, &round_id.to_le_bytes()],
        bump = round.escrow_bump,
    )]
    pub escrow_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(round_id: u64, prediction_index: u16)]
pub struct CancelPrediction<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, &round_id.to_le_bytes()],
        bump = round.bump,
    )]
    pub round: Account<'info, Round>,
    #[account(
        mut,
        seeds = [
            PREDICTION_SEED,
            &round_id.to_le_bytes(),
            user.key().as_ref(),
            &prediction_index.to_le_bytes(),
        ],
        bump = prediction.bump,
    )]
    pub prediction: Account<'info, Prediction>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [ESCROW_SEED, &round_id.to_le_bytes()],
        bump = round.escrow_bump,
    )]
    pub escrow_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BeginResolution<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority @ ErrorCode::Unauthorized,
    )]
    pub config: Account<'info, Config>,
    #[account(mut, seeds = [ROUND_SEED, &round.round_id.to_le_bytes()], bump = round.bump)]
    pub round: Account<'info, Round>,
}

#[derive(Accounts)]
pub struct SettlePrediction<'info> {
    #[account(mut, seeds = [ROUND_SEED, &round.round_id.to_le_bytes()], bump = round.bump)]
    pub round: Account<'info, Round>,
    #[account(mut)]
    pub prediction: Account<'info, Prediction>,
    #[account(mut, seeds = [ESCROW_SEED, &round.round_id.to_le_bytes()], bump = round.escrow_bump)]
    pub escrow_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub recipient_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RefundPrediction<'info> {
    #[account(seeds = [ROUND_SEED, &round.round_id.to_le_bytes()], bump = round.bump)]
    pub round: Account<'info, Round>,
    #[account(mut)]
    pub prediction: Account<'info, Prediction>,
    #[account(mut, seeds = [ESCROW_SEED, &round.round_id.to_le_bytes()], bump = round.escrow_bump)]
    pub escrow_vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct FinalizeRound<'info> {
    pub settlement_authority: Signer<'info>,
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [ROUND_SEED, &round.round_id.to_le_bytes()],
        bump = round.bump,
        constraint = settlement_authority.key() == config.settlement_authority @ ErrorCode::Unauthorized
    )]
    pub round: Account<'info, Round>,
}

#[derive(Accounts)]
pub struct MarkRoundRefunded<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority @ ErrorCode::Unauthorized,
    )]
    pub config: Account<'info, Config>,
    #[account(mut, seeds = [ROUND_SEED, &round.round_id.to_le_bytes()], bump = round.bump)]
    pub round: Account<'info, Round>,
}

#[account]
pub struct Config {
    pub authority: Pubkey,
    pub settlement_authority: Pubkey,
    pub token_mint: Pubkey,
    pub fee_treasury: Pubkey,
    pub fee_bps: u16,
    pub bump: u8,
}

impl Config {
    pub const SPACE: usize = 8  // discriminator
        + 32 // authority
        + 32 // settlement_authority
        + 32 // token_mint
        + 32 // fee_treasury
        + 2 // fee_bps
        + 1; // bump
}

#[account]
pub struct Round {
    pub round_id: u64,
    pub start_ts: i64,
    pub end_ts: i64,
    pub status: u8,
    pub token_mint: Pubkey,
    pub escrow_vault: Pubkey,
    pub total_stake: u64,
    pub total_paid: u64,
    pub final_price: Option<i64>,
    pub settlement_timestamp: Option<i64>,
    pub pyth_price_account: Pubkey,
    pub arcium_comp_id: Option<Pubkey>,
    pub result_commitment: Option<[u8; 32]>,
    pub bump: u8,
    pub escrow_bump: u8,
}

impl Round {
    pub const SPACE: usize = 8  // discriminator
        + 8  // round_id
        + 8  // start_ts
        + 8  // end_ts
        + 1  // status
        + 32 // token_mint
        + 32 // escrow_vault
        + 8  // total_stake
        + 8  // total_paid
        + (1 + 8) // final_price option
        + (1 + 8) // settlement_timestamp option
        + 32 // pyth price account
        + (1 + 32) // arcium comp id option
        + (1 + 32) // result commitment option
        + 1 // bump
        + 1; // escrow bump
}

#[account]
pub struct Prediction {
    pub round: Pubkey,
    pub owner: Pubkey,
    pub token_mint: Pubkey,
    pub commitment: [u8; 32],
    pub stake: u64,
    pub window_index: u8,
    pub status: u8,
    pub prediction_index: u16,
    pub bump: u8,
}

impl Prediction {
    pub const SPACE: usize = 8  // discriminator
        + 32 // round
        + 32 // owner
        + 32 // token mint
        + 32 // commitment
        + 8  // stake
        + 1  // window index
        + 1  // status
        + 2  // prediction index
        + 1; // bump
}

#[repr(u8)]
pub enum RoundStatus {
    Open = 0,
    Resolving = 1,
    Finalized = 2,
    Refunded = 3,
}

#[repr(u8)]
pub enum PredictionStatus {
    Submitted = 0,
    Cancelled = 1,
    Settled = 2,
    Refunded = 3,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Fee basis points must not exceed 10000")]
    InvalidFeeBps,
    #[msg("Round timing window is invalid")]
    InvalidRoundWindow,
    #[msg("Round is already active or overlaps current time")]
    RoundAlreadyActive,
    #[msg("Round is not open")]
    RoundNotOpen,
    #[msg("Round has not started")]
    RoundNotStarted,
    #[msg("Round is closed for new predictions")]
    RoundClosed,
    #[msg("Stake amount must be positive")]
    InvalidStakeAmount,
    #[msg("Mathematical overflow")]
    NumericalOverflow,
    #[msg("Caller not authorised")]
    Unauthorized,
    #[msg("Prediction already finalized")]
    PredictionFinalized,
    #[msg("Round not awaiting settlement")]
    RoundNotResolving,
    #[msg("Round mismatch between accounts")]
    RoundMismatch,
    #[msg("Insufficient escrow funds for settlement")]
    InsufficientEscrow,
    #[msg("Commitment mismatch for prediction")]
    CommitmentMismatch,
    #[msg("Round already settled")]
    RoundAlreadySettled,
    #[msg("Round is in invalid state for this operation")]
    InvalidRoundState,
}
