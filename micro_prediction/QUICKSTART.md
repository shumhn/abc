# Quick Start Guide 🚀

Get your micro-prediction app running in minutes!

## Prerequisites Check

```bash
# Check if tools are installed
solana --version   # Need: 1.18+
anchor --version   # Need: 0.31+
node --version     # Need: 18+
```

## Step 1: Install Dependencies (if needed)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install Solana
sh -c "$(curl -sSfL https://release.solana.com/stable/install)"

# Install Anchor
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
avm install latest
avm use latest

# Install Node dependencies
cd micro_prediction
yarn install
```

## Step 2: Configure Solana CLI

```bash
# Set to devnet
solana config set --url devnet

# Create a new keypair (or use existing)
solana-keygen new --outfile ~/.config/solana/id.json

# Get some SOL for testing
solana airdrop 2

# Check balance
solana balance
```

## Step 3: Build the Program

```bash
cd micro_prediction

# Build Anchor program
anchor build

# This creates: target/deploy/micro_prediction.so
```

## Step 4: Get Your Program ID

```bash
# Display the program ID
solana address -k target/deploy/micro_prediction-keypair.json

# Copy this address!
```

## Step 5: Update Program ID

Update in TWO places:

**1. `programs/micro_prediction/src/lib.rs`:**
```rust
declare_id!("YOUR_PROGRAM_ID_HERE");
```

**2. `Anchor.toml`:**
```toml
[programs.devnet]
micro_prediction = "YOUR_PROGRAM_ID_HERE"
```

## Step 6: Rebuild & Deploy

```bash
# Rebuild with new ID
anchor build

# Deploy to devnet
anchor deploy --provider.cluster devnet

# You should see: "Program Id: YOUR_PROGRAM_ID"
```

## Step 7: Run Tests

```bash
# Run all tests
anchor test --skip-deploy

# Run specific test file
yarn test tests/delegation.test.ts

# Note: Delegation tests need MagicBlock infrastructure
# Some tests may fail - that's expected without live MagicBlock
```

## Step 8: Initialize the Program

```bash
# Create a test token mint
spl-token create-token

# Save the mint address, then create an account
spl-token create-account <MINT_ADDRESS>

# Mint some tokens for testing
spl-token mint <MINT_ADDRESS> 1000000

# Now initialize the prediction market
anchor run initialize
```

## Quick Test Flow

```typescript
// In tests/ or a script:
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";

// 1. Initialize global state
await program.methods
  .initialize(
    new anchor.BN(180),  // 3 minutes
    100,                 // max 100 predictions per round
    3                    // max 3 predictions per user
  )
  .rpc();

// 2. Start a round
await program.methods
  .initializeRound(new anchor.BN(1))
  .rpc();

// 3. Place a prediction
await program.methods
  .placePrediction(
    new anchor.BN(1),           // round_id
    new anchor.BN(100_000_000), // predicted price
    new anchor.BN(10_000_000)   // stake amount
  )
  .rpc();

// 4. After 3 minutes, close round
await program.methods
  .closeRound(null)
  .accounts({
    // ... include Pyth price feed account
  })
  .rpc();

// 5. Winners claim rewards
await program.methods
  .claimReward(new anchor.BN(1))
  .rpc();
```

## Testing Delegation (Requires MagicBlock)

```bash
# Update these constants in your code with actual MagicBlock IDs:
DELEGATION_PROGRAM_ID = "..." # From MagicBlock docs
MAGIC_PROGRAM_ID = "..."
MAGIC_CONTEXT_ID = "..."

# Change RPC endpoint
export ANCHOR_PROVIDER_URL=https://devnet.magicblock.app

# Run delegation test
anchor test -- delegation
```

## Troubleshooting

### Program Build Fails
```bash
# Clean and rebuild
anchor clean
anchor build
```

### Insufficient SOL
```bash
# Get more devnet SOL
solana airdrop 2
```

### Program Already Deployed
```bash
# Check current deployment
solana program show <PROGRAM_ID>

# Close existing deployment to recover SOL
solana program close <PROGRAM_ID>

# Then redeploy
anchor deploy
```

### Tests Fail
```bash
# Make sure your local validator is NOT running
solana-test-validator --help # Don't run this
pkill solana-test-validator   # Kill if running

# Anchor test starts its own validator
anchor test
```

## Environment Variables

Create `.env` file:
```bash
ANCHOR_PROVIDER_URL=https://api.devnet.solana.com
ANCHOR_WALLET=~/.config/solana/id.json

# Optional: MagicBlock
MAGICBLOCK_RPC=https://devnet.magicblock.app

# Optional: Pyth
PYTH_SOL_USD_FEED=Gnt27xtC473ZT2Mw5u8wZ68Z3gULkSTb5DuxJy7eJotD
```

## What's Next?

1. ✅ **Program deployed** → Now test the basic flow
2. 📊 **Add Pyth integration** → See DELEGATION_INTEGRATION.md
3. ⚡ **Enable MagicBlock** → Test delegation flow
4. 🔒 **Add Arcium** → Enable encrypted predictions
5. 🎨 **Build frontend** → Create React UI

## Common Commands Reference

```bash
# Build
anchor build

# Deploy
anchor deploy

# Test
anchor test

# Show program logs
solana logs <PROGRAM_ID>

# Check program size
ls -lh target/deploy/micro_prediction.so

# Get program account info
solana program show <PROGRAM_ID>

# Verify program
anchor verify <PROGRAM_ID>
```

## Need Help?

1. Check logs: `solana logs <PROGRAM_ID>` in another terminal
2. View transaction: Copy tx signature to Solana Explorer
3. Read docs: See DELEGATION_INTEGRATION.md and NEXT_STEPS.md
4. Ask community: Solana Discord, Anchor Discord

---

**You're ready to go! Start with Step 1 above. 🎉**

