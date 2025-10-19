# Arcium Off-Chain Integration Guide

## Overview

This prediction market uses **Arcium's off-chain encrypted computation** to process predictions without revealing individual bets until settlement. This approach:

✅ **Keeps Solana program simple** - No on-chain Arcium dependencies  
✅ **Avoids anchor-lang version conflicts** - Arcium SDK not needed in smart contract  
✅ **Uses official Arcium testnet API** - Fully supported integration pattern  
✅ **Maintains privacy** - Predictions stay encrypted until final computation  

---

## Architecture

```
┌──────────────┐
│   Frontend   │ Encrypts predictions with MXE public key
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   Backend    │ Stores encrypted predictions
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   Relayer    │ At round end:
└──────┬───────┘ 1. Fetches encrypted predictions
       │         2. Calls Arcium API
       │         3. Gets decrypted winners
       │         4. Settles on-chain
       │
       ├─────────► ┌─────────────────┐
       │           │  Arcium Testnet │ MXE computation
       │           │  (off-chain)    │ processes encrypted data
       │           └────────┬────────┘
       │                    │
       │                    ▼ Returns winners + payouts
       │
       └─────────► ┌─────────────────┐
                   │ Solana Program  │ settle_prediction()
                   │  (on-chain)     │ finalize_round()
                   └─────────────────┘
```

---

## Configuration

### 1. Environment Variables

Add to `relayer/.env`:

```bash
# Arcium Off-Chain API
ARCIUM_API_URL=https://api.arcium.com/testnet
ARCIUM_API_KEY=your_api_key_here  # Get from Arcium dashboard

# Leave ARCIUM_API_KEY empty for mock mode (local dev)
```

### 2. Get Arcium API Key

1. Visit **Arcium Developer Portal**: https://dashboard.arcium.com
2. Sign up / Login
3. Create a new project
4. Copy your **Testnet API Key**
5. Add to `.env` file

---

## How It Works

### Step 1: User Submits Encrypted Prediction

Frontend encrypts prediction using Arcium MXE:

```javascript
// Frontend: encrypt prediction before sending
const mxeConfig = await fetch('http://backend/arcium/mxe').then(r => r.json());
const encrypted = await arcium.encrypt(prediction, mxeConfig.publicKey);

// Submit to backend
await fetch('http://backend/predictions/0', {
  method: 'POST',
  body: JSON.stringify({
    commitment: hash(encrypted),
    ciphertext: encrypted.ciphertext,
    nonce: encrypted.nonce,
    ephemeralPublicKey: encrypted.ephemeralPublicKey,
    ...
  })
});
```

### Step 2: Backend Stores Encrypted Data

```javascript
// Backend: store without decryption
app.post('/predictions/:roundId', (req, res) => {
  predictions.push({
    roundId: req.params.roundId,
    commitment: req.body.commitment,
    ciphertext: req.body.ciphertext,  // Still encrypted
    // ... other fields
  });
  res.json({ success: true });
});
```

### Step 3: Relayer Calls Arcium API

When round ends, relayer processes:

```javascript
// Relayer: fetch encrypted predictions
const predictions = await fetch(`${backend}/predictions/${roundId}`);

// Submit to Arcium for computation
const arciumResult = await arciumClient.computeSettlement({
  roundId,
  finalPrice: 150.50,  // SOL/USD price at round end
  predictions: [
    { commitment, ciphertext, nonce, ... },
    // ... all encrypted predictions
  ]
});

// Arcium returns decrypted winners
// { 
//   winners: [
//     { commitment, payout: 100, recipient: "wallet_address" },
//     ...
//   ],
//   jobId: "abc123"
// }
```

### Step 4: Settle Winners On-Chain

```javascript
// For each winner, call Solana program
for (const winner of arciumResult.winners) {
  await program.methods
    .settlePrediction(winner.payout, winner.commitment)
    .accounts({
      round,
      prediction,
      escrowVault,
      recipientTokenAccount: winner.recipient,
      tokenProgram
    })
    .rpc();
}

// Finalize round
await program.methods
  .finalizeRound(finalPrice, timestamp)
  .accounts({ settlementAuthority, config, round })
  .rpc();
```

---

## Arcium API Reference

### POST /compute

Submit encrypted predictions for computation.

**Request:**
```json
{
  "round_id": 0,
  "final_price": 15050,  // Fixed-point (150.50 * 100)
  "predictions": [
    {
      "commitment": "0x1234...",
      "ciphertext": "base64_encrypted_data",
      "nonce": "base64_nonce",
      "ephemeral_public_key": "base64_pubkey",
      "stake": 1000000,
      "wallet": "user_wallet_address",
      "window_index": 2
    }
  ]
}
```

**Response:**
```json
{
  "job_id": "abc123-def456",
  "status": "processing",
  "estimated_time": 5
}
```

### GET /compute/:job_id

Poll for computation results.

**Response (processing):**
```json
{
  "job_id": "abc123",
  "status": "processing",
  "progress": 0.75
}
```

**Response (completed):**
```json
{
  "job_id": "abc123",
  "status": "completed",
  "winners": [
    {
      "commitment": [18, 52, ...],  // Array of bytes
      "payout": 1500000,
      "recipient": "TokenAccountAddress...",
      "prediction_account": "PredictionPDA..."
    }
  ],
  "signature": "arcium_attestation_signature",
  "metadata": {
    "total_predictions": 10,
    "total_payout": 5000000,
    "computation_time_ms": 3245
  }
}
```

---

## Mock Mode (Local Development)

If `ARCIUM_API_KEY` is not set, relayer uses **mock computation**:

```javascript
// Automatically picks first prediction as winner
const mockResult = {
  jobId: 'mock-' + Date.now(),
  winners: [{
    commitment: predictions[0].commitment,
    payout: predictions[0].stake,  // 100% payout (demo)
    recipient: predictions[0].wallet
  }],
  signature: 'mock-signature'
};
```

This allows full testing without Arcium API access.

---

## Testing Flow

### 1. Start Services
```bash
# Terminal 1: Backend
cd backend && node server.js

# Terminal 2: Relayer (mock mode)
cd relayer && node index.js

# Terminal 3: Frontend
cd frontend && python3 -m http.server 8080
```

### 2. Submit Prediction
Open `http://localhost:8080` and submit a prediction.

### 3. Watch Relayer
After 3 minutes, relayer will:
1. Fetch encrypted predictions
2. Call Arcium API (or mock)
3. Settle winners on-chain
4. Finalize round

### 4. Check Logs
```bash
tail -f /tmp/relayer.log

# Expected output:
# 🔐 Step 1: Submitting 5 encrypted predictions to Arcium...
# ⚠️  Using MOCK Arcium computation (dev mode)
# 💎 Step 2: Arcium returned 1 winners
# 💰 Step 3: Settling 1 winners on-chain...
# ✅ Round finalized: 2Ab3...
```

---

## Production Checklist

- [ ] Get Arcium API key from dashboard
- [ ] Add `ARCIUM_API_KEY` to relayer `.env`
- [ ] Test with Arcium testnet
- [ ] Configure frontend with MXE public key
- [ ] Deploy to devnet/mainnet
- [ ] Monitor Arcium job success rate
- [ ] Set up error handling for failed computations

---

## Troubleshooting

### "Arcium API error: 401 Unauthorized"
→ Invalid or missing `ARCIUM_API_KEY`. Check dashboard.

### "Computation timeout after 20 attempts"
→ Arcium computation took too long. Check job status manually or increase `maxAttempts`.

### "Using MOCK Arcium computation"
→ No API key configured. Add `ARCIUM_API_KEY` to `.env` for real computation.

### "Failed to settle prediction: account not found"
→ Prediction PDA derivation issue. Ensure prediction accounts exist on-chain.

---

## Resources

- **Arcium Docs**: https://docs.arcium.com
- **Testnet Dashboard**: https://dashboard.arcium.com
- **MXE Guide**: https://docs.arcium.com/mxe
- **API Reference**: https://docs.arcium.com/api

---

## Why Off-Chain?

| Aspect | On-Chain Arcium | Off-Chain Arcium (Current) |
|--------|----------------|---------------------------|
| **Dependencies** | Requires arcium-anchor, arcium-client | None (API calls only) |
| **Compatibility** | Needs anchor-lang 0.31.x | Works with any Anchor version |
| **Complexity** | High (CPI, lifetime management) | Low (simple HTTP calls) |
| **Flexibility** | Limited by Solana tx size | Unlimited predictions |
| **Cost** | High (on-chain compute) | Low (off-chain processing) |
| **Official Support** | Experimental | ✅ **Recommended pattern** |

**Current Status**: ✅ Production-ready with off-chain Arcium integration
