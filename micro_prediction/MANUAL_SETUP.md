# Manual Setup Guide (If Script Fails)

Your Rust is already installed! ✅

Follow these steps to complete the setup:

---

## Step 1: Install Solana CLI

```bash
sh -c "$(curl -sSfL https://release.solana.com/stable/install)"
```

Then add to your PATH:
```bash
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
```

Verify:
```bash
solana --version
```

---

## Step 2: Install Anchor

```bash
# Add cargo bin to PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Install Anchor Version Manager
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force

# Install latest Anchor
avm install latest
avm use latest

# Verify
anchor --version
```

---

## Step 3: Configure Solana

```bash
# Set to devnet
solana config set --url devnet

# Create keypair (if you don't have one)
solana-keygen new --outfile ~/.config/solana/id.json

# Check your address
solana address

# Airdrop SOL
solana airdrop 2
```

---

## Step 4: Build & Deploy

```bash
cd /Users/sumangiri/Desktop/pre/micro_prediction

# Install Node dependencies
yarn install

# Build the program (this takes 2-5 minutes)
anchor build

# Get your Program ID
solana address -k target/deploy/micro_prediction-keypair.json
```

**IMPORTANT:** Copy this Program ID and update it in:

1. `programs/micro_prediction/src/lib.rs` line 27:
   ```rust
   declare_id!("YOUR_PROGRAM_ID_HERE");
   ```

2. `Anchor.toml` line 17:
   ```toml
   micro_prediction = "YOUR_PROGRAM_ID_HERE"
   ```

Then rebuild and deploy:
```bash
anchor build
anchor deploy --provider.cluster devnet
```

---

## Step 5: Test

```bash
anchor test --skip-deploy
```

---

## Troubleshooting

**If anchor command not found:**
```bash
export PATH="$HOME/.cargo/bin:$PATH"
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
```

**If solana command not found:**
```bash
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
echo 'export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"' >> ~/.zshrc
```

**If airdrop fails:**
Visit: https://faucet.solana.com/ and paste your address

---

## Quick Reference

```bash
# Check balance
solana balance

# View program
solana program show <PROGRAM_ID>

# View logs (in separate terminal)
solana logs <PROGRAM_ID>

# Check cluster
solana config get
```

---

**After deployment succeeds, you can start building the frontend!**

