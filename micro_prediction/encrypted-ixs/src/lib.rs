use arcis_imports::*;

#[encrypted]
mod circuits {
    use arcis_imports::*;

    // Original add_together circuit for testing
    pub struct InputValues {
        v1: u8,
        v2: u8,
    }

    #[instruction]
    pub fn add_together(input_ctxt: Enc<Shared, InputValues>) -> Enc<Shared, u16> {
        let input = input_ctxt.to_arcis();
        let sum = input.v1 as u16 + input.v2 as u16;
        input_ctxt.owner.from_arcis(sum)
    }

    // Prediction market circuits
    pub struct PredictionInput {
        user_prediction: u64,
        actual_price: u64,
        threshold_percent: u8, // e.g., 5 for 5% threshold
    }

    pub struct PredictionOutput {
        is_winner: bool,
        absolute_diff: u64,
    }

    /// Check if a prediction is within threshold of actual price
    #[instruction]
    pub fn check_prediction_winner(
        input_ctxt: Enc<Shared, PredictionInput>
    ) -> Enc<Shared, PredictionOutput> {
        let input = input_ctxt.to_arcis();
        
        // Calculate absolute difference
        let diff = if input.user_prediction > input.actual_price {
            input.user_prediction - input.actual_price
        } else {
            input.actual_price - input.user_prediction
        };
        
        // Calculate threshold (e.g., 5% of actual price)
        let threshold = (input.actual_price * input.threshold_percent as u64) / 100;
        
        // Determine if winner
        let is_winner = diff <= threshold;
        
        let output = PredictionOutput {
            is_winner,
            absolute_diff: diff,
        };
        
        input_ctxt.owner.from_arcis(output)
    }

    pub struct BatchPredictionsInput {
        predictions: [u64; 10], // Support up to 10 predictions
        actual_price: u64,
        num_predictions: u8,
    }

    pub struct BatchPredictionsOutput {
        winner_indices: [u8; 10],
        differences: [u64; 10],
        num_winners: u8,
    }

    /// Batch process multiple predictions and find winners
    #[instruction]
    pub fn batch_check_winners(
        input_ctxt: Enc<Shared, BatchPredictionsInput>
    ) -> Enc<Shared, BatchPredictionsOutput> {
        let input = input_ctxt.to_arcis();
        
        let mut winner_indices = [0u8; 10];
        let mut differences = [0u64; 10];
        let mut num_winners = 0u8;
        let mut min_diff = u64::MAX;
        
        // First pass: find minimum difference
        for i in 0..(input.num_predictions as usize) {
            let prediction = input.predictions[i];
            let diff = if prediction > input.actual_price {
                prediction - input.actual_price
            } else {
                input.actual_price - prediction
            };
            differences[i] = diff;
            
            if diff < min_diff {
                min_diff = diff;
            }
        }
        
        // Second pass: mark winners (those with minimum difference)
        for i in 0..(input.num_predictions as usize) {
            if differences[i] == min_diff {
                winner_indices[num_winners as usize] = i as u8;
                num_winners += 1;
            }
        }
        
        let output = BatchPredictionsOutput {
            winner_indices,
            differences,
            num_winners,
        };
        
        input_ctxt.owner.from_arcis(output)
    }

    pub struct PrivatePriceInput {
        encrypted_price_1: u64,
        encrypted_price_2: u64,
        encrypted_price_3: u64,
        actual_price: u64,
    }

    pub struct PrivatePriceOutput {
        closest_index: u8, // 1, 2, or 3
        min_difference: u64,
    }

    /// Compare 3 private predictions and find closest to actual price
    #[instruction]
    pub fn find_closest_prediction(
        input_ctxt: Enc<Shared, PrivatePriceInput>
    ) -> Enc<Shared, PrivatePriceOutput> {
        let input = input_ctxt.to_arcis();
        
        let prices = [
            input.encrypted_price_1,
            input.encrypted_price_2,
            input.encrypted_price_3,
        ];
        
        let mut min_diff = u64::MAX;
        let mut closest_idx = 0u8;
        
        for (idx, &price) in prices.iter().enumerate() {
            let diff = if price > input.actual_price {
                price - input.actual_price
            } else {
                input.actual_price - price
            };
            
            if diff < min_diff {
                min_diff = diff;
                closest_idx = (idx + 1) as u8;
            }
        }
        
        let output = PrivatePriceOutput {
            closest_index: closest_idx,
            min_difference: min_diff,
        };
        
        input_ctxt.owner.from_arcis(output)
    }
}
