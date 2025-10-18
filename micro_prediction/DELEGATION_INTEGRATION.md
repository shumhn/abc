# MagicBlock Delegation Integration Guide

## Overview

This document explains how the MagicBlock Ephemeral Rollups delegation has been integrated into the micro-prediction program and how to use it with Arcium and the frontend.

## Program Changes

### New Instructions

1. **`delegate_round(round_id, config)`**
   - Delegates the round state and prediction ledger accounts to MagicBlock's Ephemeral Rollup
   - Must be called while round status is `Open`
   - Creates delegation tracking PDAs (`DelegatedRoundState` and `DelegatedLedgerState`)
   - Sets `delegation_status` to `Delegated`
   - Config options:
     - `commit_frequency_ms`: How often to auto-commit state (optional)
     - `validator`: Specific validator to use (optional)

2. **`commit_round(round_id)`**
   - Commits the current state of delegated accounts back to Solana base layer
   - Must be called after round is `Settled` and delegation is active
   - Does not undelegate the accounts (they remain in the Ephemeral Rollup)

3. **`commit_and_undelegate_round(round_id)`**
   - Commits state and schedules undelegation in one transaction
   - Must be called after round is `Settled`
   - Sets `delegation_status` to `CommitScheduled`
   - More efficient than calling commit and undelegate separately

4. **`undelegate_round(round_id)`**
   - Returns delegated accounts back to program control
   - Closes the delegation tracking PDAs
   - Sets `delegation_status` back to `NotDelegated`
   - Can be called after commit or independently

### State Changes

- **RoundState**: Added `delegation_status: DelegationStatus` field
- **DelegationStatus enum**:
  - `NotDelegated`: Default state, accounts under program control
  - `Delegated`: Accounts delegated to Ephemeral Rollup
  - `CommitScheduled`: Commit and undelegation have been scheduled

### New Error Codes

- `DelegationAlreadyActive`: Attempted to delegate when already delegated
- `DelegationNotActive`: Attempted delegation operation when not delegated
- `RoundDelegationInvalidStatus`: Round in wrong status for delegation operation

## Typical Workflow

### Standard Flow (with delegation)

```
1. initialize_round(round_id)
   → Status: Open, delegation_status: NotDelegated

2. delegate_round(round_id, config)
   → Status: Open, delegation_status: Delegated
   → Accounts now in Ephemeral Rollup (sub-10ms latency)

3. place_prediction(...) [called by users, multiple times]
   → Ultra-fast execution in Ephemeral Rollup
   → No gas fees during rollup session

4. close_round()
   → Status: Settled, delegation_status: Delegated
   → Fetches Pyth price, calculates winners

5. commit_and_undelegate_round(round_id)
   → Commits final state to base layer
   → delegation_status: CommitScheduled

6. claim_reward(round_id) [called by winners]
   → Winners claim their rewards
```

### Alternative: Manual Commit Flow

```
1-4. [Same as above]

5. commit_round(round_id)
   → Commits state but keeps delegation active

6. undelegate_round(round_id)
   → Returns accounts to program control
   → delegation_status: NotDelegated

7. claim_reward(round_id)
```

## Frontend Integration

### 1. Connect to MagicBlock RPC

```typescript
import { Connection } from '@solana/web3.js';

// Use MagicBlock's devnet endpoint
const MAGICBLOCK_RPC = 'https://devnet.magicblock.app';
const connection = new Connection(MAGICBLOCK_RPC, 'confirmed');
```

### 2. Derive Delegation PDAs

```typescript
import { PublicKey } from '@solana/web3.js';

const PROGRAM_ID = new PublicKey('3btqev6Y8xNxqwFxFKaDPihQyVZ1gs2DpBNsDukmHxNX');
const DELEGATION_PROGRAM_ID = new PublicKey('<DELEGATION_PROGRAM_ID>');

// Derive delegation tracking PDAs
const [delegatedRoundState] = PublicKey.findProgramAddressSync(
  [Buffer.from('magic-delegated-state'), roundIdBuffer],
  PROGRAM_ID
);

const [delegatedLedgerState] = PublicKey.findProgramAddressSync(
  [Buffer.from('magic-delegated-ledger'), roundIdBuffer],
  PROGRAM_ID
);

// Use DelegateAccounts helper from SDK
import { DelegateAccounts } from 'ephemeral-rollups-sdk';

const roundDelegateAccounts = DelegateAccounts.new(
  roundStatePubkey,
  PROGRAM_ID
);
// roundDelegateAccounts will contain: delegate_buffer, delegation_record, delegation_metadata

const ledgerDelegateAccounts = DelegateAccounts.new(
  predictionLedgerPubkey,
  PROGRAM_ID
);
```

### 3. Call Delegation Instructions

```typescript
// Delegate round
await program.methods
  .delegateRound(roundId, {
    commitFrequencyMs: 5000, // commit every 5 seconds
    validator: null,
  })
  .accounts({
    authority: authorityPubkey,
    globalState,
    roundState,
    predictionLedger,
    delegatedRoundState,
    delegatedLedgerState,
    ownerProgram: PROGRAM_ID,
    roundDelegationBuffer: roundDelegateAccounts.delegate_buffer,
    roundDelegationRecord: roundDelegateAccounts.delegation_record,
    roundDelegationMetadata: roundDelegateAccounts.delegation_metadata,
    ledgerDelegationBuffer: ledgerDelegateAccounts.delegate_buffer,
    ledgerDelegationRecord: ledgerDelegateAccounts.delegation_record,
    ledgerDelegationMetadata: ledgerDelegateAccounts.delegation_metadata,
    delegationProgram: DELEGATION_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  })
  .rpc();

// Place predictions (now lightning-fast!)
await program.methods
  .placePrediction(roundId, predictedPrice, stakeAmount)
  .accounts({...})
  .rpc();

// After round closes, commit and undelegate
await program.methods
  .commitAndUndelegateRound(roundId)
  .accounts({
    authority: authorityPubkey,
    globalState,
    roundState,
    predictionLedger,
    magicContext: MAGIC_CONTEXT_ID,
    magicProgram: MAGIC_PROGRAM_ID,
  })
  .rpc();
```

## Arcium Integration for Encrypted Predictions

### Circuit Example

In `encrypted-ixs/src/lib.rs`, define a circuit for encrypted price checking:

```rust
use arcis_imports::*;

#[encrypted]
mod circuits {
    use arcis_imports::*;

    pub struct PredictionInput {
        user_prediction: u64,
        actual_price: u64,
    }

    #[instruction]
    pub fn check_winner(input_ctxt: Enc<Shared, PredictionInput>) -> Enc<Shared, bool> {
        let input = input_ctxt.to_arcis();
        let diff = if input.user_prediction > input.actual_price {
            input.user_prediction - input.actual_price
        } else {
            input.actual_price - input.user_prediction
        };
        
        // Winner if within 1% of actual price
        let threshold = input.actual_price / 100;
        let is_winner = diff <= threshold;
        
        input_ctxt.owner.from_arcis(is_winner)
    }
}
```

### Frontend: Encrypt Predictions

```typescript
import { X25519KeyPair, RescueCipher } from '@arcium-hq/client';

// Generate keypair for encryption
const userKeypair = X25519KeyPair.generate();

// Get MXE public key (from Arcium)
const mxePublicKey = await getMXEPublicKey();

// Derive shared secret
const sharedSecret = userKeypair.deriveSharedSecret(mxePublicKey);

// Encrypt prediction
const nonce = generateRandomNonce();
const cipher = new RescueCipher(sharedSecret);
const ciphertext = cipher.encrypt(
  Buffer.from(predictedPrice.toString()),
  nonce
);

// Submit encrypted prediction
await program.methods
  .placePredictionEncrypted(
    roundId,
    Array.from(ciphertext),
    Array.from(userKeypair.publicKey),
    nonce,
    stakeAmount
  )
  .accounts({...})
  .rpc();
```

## Performance Benefits

- **Without delegation**: ~400-600ms transaction confirmation on Solana devnet
- **With delegation**: ~5-15ms execution in Ephemeral Rollup
- **Gas fees**: Eliminated during rollup session (only pay for delegate/commit/undelegate)

## Testing

1. Deploy program with delegation support
2. Initialize a round normally
3. Call `delegate_round` before users start betting
4. Place multiple predictions (should be instant)
5. Close round and verify winners calculated correctly
6. Call `commit_and_undelegate_round`
7. Verify winners can claim rewards
8. Check that delegation PDAs are closed

## Notes

- Delegation is optional; rounds can run without it
- Commit can be automatic via `commit_frequency_ms` config
- Always undelegate before closing accounts to avoid locked funds
- MagicBlock handles state synchronization automatically
- Pyth price feeds work normally during delegation

## Resources

- MagicBlock Docs: https://docs.magicblock.gg/
- Ephemeral Rollups SDK: https://github.com/magicblock-labs/delegation-program
- Arcium Docs: https://docs.arcium.com/
- Pyth Network: https://docs.pyth.network/

