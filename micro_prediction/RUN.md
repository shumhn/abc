# 🚀 Encrypted Prediction Market - Quick Start

## System Overview

A repeating 3-minute prediction market where users submit encrypted predictions and stake tokens. Predictions are encrypted client-side with Arcium keys and recorded as commitments on-chain while funds are escrowed in program PDAs. At round end, final price from Pyth is used to compute winners privately via Arcium MPC, which returns a verifiable settlement payload that the relayer submits to settle and distribute funds.

## Architecture

```
┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│   Next.js   │─────▶│   Anchor     │─────▶│  Escrow PDA │
│  Frontend   │      │   Program    │      │   (Tokens)  │
└─────────────┘      └──────────────┘      └─────────────┘
      │                      │                      │
      │                      │                      │
      ▼                      ▼                      ▼
┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│   Arcium    │      │     Pyth     │      │   Relayer   │
│  Encryption │      │    Oracle    │      │ Orchestrator│
└─────────────┘      └──────────────┘      └─────────────┘
```

## 📋 Prerequisites

- Node.js 18+
- Solana CLI tools
- Anchor 0.32+
- Rust toolchain
- Local Solana validator (for testing)

## 🛠️ Setup Instructions

### 1. Install Dependencies

```bash
# Root project
npm install

# Frontend app
cd app
npm install

# Relayer
cd ../relayer
npm install
```

### 2. Build Anchor Program

```bash
# From project root
anchor build
```

### 3. Deploy Program (Local)

```bash
# Start local validator
solana-test-validator

# In another terminal, deploy
anchor deploy
```

### 4. Initialize Program

```bash
# Update program ID in:
# - Anchor.toml
# - app/src/components/PredictionForm.tsx (PROGRAM_ID)
# - relayer/index.js (PROGRAM_ID)

# Run initialization script
anchor run initialize
```

### 5. Configure Environment

```bash
# Frontend
cd app
cp .env.example .env.local
# Edit .env.local with your RPC URL

# Relayer
cd ../relayer
cp .env.example .env
# Edit .env with your keypair path

# Backend
cd ../backend
cp .env.example .env
# Populate required values (see below)
```

#### Required Environment Variables

- **`BACKEND_PORT`** (`backend/.env`) – Port for the Express API (default `3001`).
- **`ANCHOR_PROVIDER_URL`** (`backend/.env`) – RPC endpoint the backend uses for read-only queries.
- **`RPC_URL`** (`relayer/.env`) – RPC endpoint the relayer uses for transactions.
- **`KEYPAIR_PATH`** (`relayer/.env`) – Filesystem path to the authority keypair used by the relayer.
- **`BACKEND_URL`** (`relayer/.env`, `frontend/.env`) – Base URL of the backend API, e.g. `http://localhost:3001`.
- **`HELIUS_API_KEY`** (`relayer/.env`, optional) – Enables high-performance Pyth access when running against Helius RPC.
- **`NEXT_PUBLIC_RPC_URL`** (`app/.env.local`) – RPC endpoint exposed to the frontend wallet adapter.

## 🎮 Running the System

### Terminal 1: Local Validator

```bash
solana-test-validator
```

### Terminal 2: Relayer (Round Management)

```bash
cd relayer
npm start
```

Output:
```
🚀 Relayer initialized
Authority: ABC...XYZ
✅ Program loaded
📋 Initializing Round #0
✅ Round initialized
⏱️  Round #0 - 170s remaining
```

### Terminal 3: Frontend

```bash
cd app
npm run dev
```

Visit: **http://localhost:3000**

## 📝 Usage Flow

### 1. Connect Wallet
- Click "Select Wallet" button
- Choose Phantom or Solflare
- Approve connection

### 2. Submit Prediction
- Choose prediction window (Down/Hold/Up)
- Enter optional exact price
- Set stake amount (min 0.001 SOL)
- Click "🔐 Encrypt & Submit Prediction"

### 3. Encryption Flow
```
User Input → Arcium Encryption → Commitment Hash → On-chain TX → Escrow
```

### 4. Round Settlement (Automatic)
```
Round End → Pyth Price → Arcium MPC → Settlement → Payouts
```

## 🔐 Arcium Integration

The system uses Arcium for:

1. **Client-side Encryption**
   ```typescript
   // app/src/lib/arcium.ts
   const encrypted = await encryptPrediction(data, mxePublicKey);
   // Returns: ciphertext, commitment, nonce, ephemeralPublicKey
   ```

2. **MPC Computation** (Relayer)
   - Input: All encrypted predictions + final price
   - Output: Winners + payout amounts (signed)
   - Privacy: Predictions remain encrypted

3. **Verification**
   ```rust
   // programs/micro_prediction/src/lib.rs
   pub fn execute_settlement(payload, arcium_sig) {
       // Verify Arcium signature
       // Transfer funds to winners
   }
   ```

## 📊 Testing

### Unit Tests

```bash
anchor test
```

### Integration Test

1. Start validator & relayer
2. Open frontend in two browsers
3. Submit predictions from both
4. Wait for round end
5. Check settlement logs

### Check Accounts

```bash
# View round account
solana account <ROUND_PDA>

# View prediction account
solana account <PREDICTION_PDA>

# Check escrow balance
spl-token accounts
```

## 🐛 Troubleshooting

### Frontend Issues

**"Failed to fetch MXE public key"**
- Ensure program is deployed
- MXE account must be initialized
- Check RPC connection

**"Transaction failed"**
- Check wallet has SOL for fees
- Verify program ID is correct
- Round must be Open status

### Relayer Issues

**"Failed to initialize round"**
- Check authority keypair path
- Ensure config account exists
- Verify token mint is valid

**"Pyth price fetch failed"**
- Using devnet Pyth feed
- Fallback to mock price enabled
- Check network connection

## 📂 Project Structure

```
micro_prediction/
├── programs/
│   └── micro_prediction/
│       └── src/
│           └── lib.rs              # Anchor program
├── app/                            # Next.js frontend
│   ├── src/
│   │   ├── app/
│   │   │   └── page.tsx           # Main page
│   │   ├── components/
│   │   │   ├── WalletProvider.tsx # Wallet adapter
│   │   │   ├── PriceDisplay.tsx   # Pyth price feed
│   │   │   ├── RoundTimer.tsx     # Countdown timer
│   │   │   └── PredictionForm.tsx # Submit predictions
│   │   └── lib/
│   │       └── arcium.ts          # Encryption utils
│   └── public/
│       └── idl/                   # Program IDL
├── relayer/
│   └── index.js                   # Round orchestrator
├── tests/
│   └── micro_prediction.ts        # Anchor tests
└── Anchor.toml                    # Anchor config
```

## 🎯 Key Features

### ✅ Implemented

- [x] Anchor program with escrow logic
- [x] Client-side Arcium encryption
- [x] Wallet adapter integration
- [x] Pyth price feed integration
- [x] Round management relayer
- [x] Automatic round rotation
- [x] Next.js responsive UI

### 🔄 To Integrate

- [ ] Real Arcium testnet computation
- [ ] MagicBlock ephemeral rollup
- [ ] Production token mint
- [ ] Multi-token support
- [ ] Leaderboard & history

## 🔗 Resources

- **Arcium Docs**: https://docs.arcium.com
- **Pyth Network**: https://pyth.network
- **Anchor Book**: https://book.anchor-lang.com
- **Solana Cookbook**: https://solanacookbook.com

## 📄 License

ISC

---

**Built for encrypted on-chain predictions** 🔐📊
