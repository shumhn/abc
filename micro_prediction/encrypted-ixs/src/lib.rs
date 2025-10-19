use arcis_imports::*;

#[encrypted]
mod circuits {
    use arcis_imports::*;

    #[derive(Clone, Copy, MxeSerializable)]
    pub struct EncryptedPrediction {
        pub commitment: [u8; 32],
        pub predicted_price: EncScalar<Shared>,
        pub stake: EncScalar<Shared>,
    }

    #[derive(Clone, MxeSerializable)]
    pub struct SettlementEntry {
        pub commitment: [u8; 32],
        pub payout: u64,
    }

    #[derive(Clone, MxeSerializable)]
    pub struct SettlementResult {
        pub round_id: u64,
        pub final_price: i64,
        pub fee_total: u64,
        pub settlements: Vec<SettlementEntry>,
    }

    #[instruction]
    pub fn determine_winners(
        predictions: Enc<Vec<EncryptedPrediction>, Shared>,
        final_price: EncScalar<Shared>,
        fee_bps: u16,
        round_id: u64,
    ) -> Enc<SettlementResult, Shared> {
        let predictions = predictions.to_arcis();
        let final_price_val = final_price.to_arcis();

        let mut min_diff: Option<u128> = None;
        for prediction in predictions.iter() {
            let price = prediction.predicted_price.to_arcis();
            let diff = distance(price, final_price_val);
            min_diff = Some(match min_diff {
                Some(current_min) if diff < current_min => diff,
                Some(current_min) => current_min,
                None => diff,
            });
        }

        let min_diff = min_diff.unwrap_or_default();

        let mut total_stake: u128 = 0;
        let mut winners: Vec<&EncryptedPrediction> = Vec::new();
        for prediction in predictions.iter() {
            let price = prediction.predicted_price.to_arcis();
            let diff = distance(price, final_price_val);
            let stake = prediction.stake.to_arcis();
            total_stake = total_stake.saturating_add(stake);
            if diff == min_diff {
                winners.push(prediction);
            }
        }

        let mut payouts: Vec<SettlementEntry> = Vec::new();
        let mut fee_total: u128 = 0;

        if !winners.is_empty() {
            for winner in winners.iter() {
                let stake = winner.stake.to_arcis();
                let gross = stake;
                let fee = gross * fee_bps as u128 / 10_000u128;
                let payout = gross.saturating_sub(fee);
                fee_total = fee_total.saturating_add(fee);
                payouts.push(SettlementEntry {
                    commitment: winner.commitment,
                    payout: payout as u64,
                });
            }
        }

        Enc::from_arcis(
            SettlementResult {
                round_id,
                final_price: final_price_val as i64,
                fee_total: fee_total as u64,
                settlements: payouts,
            },
            predictions.owner,
        )
    }

    fn distance(a: u128, b: u128) -> u128 {
        if a >= b {
            a - b
        } else {
            b - a
        }
    }
}
