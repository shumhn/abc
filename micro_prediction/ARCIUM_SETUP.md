# Arcium Real Integration - Quick Start

## 🎯 Goal
Add real Arcium encrypted computation to your prediction market (no node setup needed).

---

## Step 1: Install Arcium TypeScript SDK (2 minutes)

### Frontend Package
```bash
cd frontend
npm install @arcium-hq/client @arcium-hq/reader
```

### Backend/Relayer Package
```bash
cd ../backend
npm install @arcium-hq/client @arcium-hq/reader

cd ../relayer
npm install @arcium-hq/client @arcium-hq/reader
```

---

## Step 2: Get MXE Configuration (No API Key Needed!)

Arcium testnet is **publicly accessible**. You just need to know which MXE (computation environment) to use.

### Check Available MXEs

Visit Arcium's public testnet explorer or use the SDK:

```javascript
// backend/arcium-setup.js
const { ArciumReader } = require('@arcium-hq/reader');
const { Connection } = require('@solana/web3.js');

async function setupArcium() {
  const connection = new Connection('https://api.devnet.solana.com');
  const reader = new ArciumReader(connection);
  
  // Get available MXEs on testnet
  const mxes = await reader.fetchAllMxes();
  console.log('Available MXEs:', mxes);
  
  // Pick one for your use case (or create your own)
  const predictionMxe = mxes.find(m => m.name.includes('prediction'));
  
  return {
    mxeId: predictionMxe?.id || mxes[0]?.id,
    publicKey: predictionMxe?.publicKey || mxes[0]?.publicKey
  };
}

setupArcium().then(config => {
  console.log('Use this MXE config:', config);
  // Save to .env or config file
});
```

Run it:
```bash
cd backend
node arcium-setup.js
```

---

## Step 3: Update Frontend to Encrypt Predictions

### Create Arcium Encryption Module

```javascript
// frontend/arcium-encrypt.js
import { ArciumClient } from '@arcium-hq/client';

class PredictionEncryption {
  constructor(mxePublicKey) {
    this.client = new ArciumClient();
    this.mxePublicKey = mxePublicKey;
  }

  async encryptPrediction(predictionData) {
    // predictionData = { windowIndex: 2, amount: 1000000 }
    const plaintext = JSON.stringify(predictionData);
    
    const encrypted = await this.client.encrypt({
      data: new TextEncoder().encode(plaintext),
      publicKey: this.mxePublicKey
    });
    
    return {
      ciphertext: encrypted.ciphertext,
      nonce: encrypted.nonce,
      ephemeralPublicKey: encrypted.ephemeralPublicKey,
      commitment: this.generateCommitment(encrypted.ciphertext)
    };
  }
  
  generateCommitment(ciphertext) {
    // Hash the ciphertext to create commitment
    const hash = crypto.subtle.digest('SHA-256', ciphertext);
    return new Uint8Array(hash);
  }
}

export default PredictionEncryption;
```

### Update Frontend Form

```javascript
// frontend/app.js (modify existing)
import PredictionEncryption from './arcium-encrypt.js';

// Initialize with MXE public key from backend
const mxeConfig = await fetch('/arcium/mxe').then(r => r.json());
const encryption = new PredictionEncryption(mxeConfig.publicKey);

// When user submits prediction
async function submitPrediction(windowIndex, amount) {
  // Encrypt the prediction
  const encrypted = await encryption.encryptPrediction({
    windowIndex,
    amount
  });
  
  // Submit to backend (already encrypted!)
  await fetch(`/predictions/${currentRound}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      commitment: Array.from(encrypted.commitment),
      ciphertext: Buffer.from(encrypted.ciphertext).toString('base64'),
      nonce: Buffer.from(encrypted.nonce).toString('base64'),
      ephemeralPublicKey: Buffer.from(encrypted.ephemeralPublicKey).toString('base64'),
      wallet: userWallet.publicKey.toString(),
      stake: amount
    })
  });
}
```

---

## Step 4: Update Backend to Store MXE Config

```javascript
// backend/server.js (add this endpoint)
const { ArciumReader } = require('@arcium-hq/reader');

// Initialize Arcium
let arciumMxeConfig = null;

async function initArcium() {
  const connection = new Connection('https://api.devnet.solana.com');
  const reader = new ArciumReader(connection);
  
  const mxes = await reader.fetchAllMxes();
  arciumMxeConfig = {
    mxeId: mxes[0]?.id,
    publicKey: mxes[0]?.publicKey?.toString()
  };
  
  console.log('Arcium MXE initialized:', arciumMxeConfig);
}

// Start server
initArcium().then(() => {
  app.listen(3001, () => {
    console.log('Backend with Arcium ready on port 3001');
  });
});

// Endpoint to get MXE config
app.get('/arcium/mxe', (req, res) => {
  res.json(arciumMxeConfig);
});
```

---

## Step 5: Update Relayer to Use Arcium Computation

Replace the mock computation with real Arcium:

```javascript
// relayer/arcium-client.js (update existing)
const { ArciumClient } = require('@arcium-hq/client');
const { Connection, PublicKey } = require('@solana/web3.js');

class ArciumComputeClient {
  constructor() {
    this.connection = new Connection('https://api.devnet.solana.com');
    this.client = new ArciumClient(this.connection);
  }

  async computeSettlement(payload) {
    console.log('🔐 Submitting to real Arcium network...');
    
    // Submit encrypted predictions to Arcium MXE
    const computation = await this.client.submitComputation({
      mxeId: payload.mxeId,
      inputs: payload.predictions.map(p => ({
        ciphertext: Buffer.from(p.ciphertext, 'base64'),
        nonce: Buffer.from(p.nonce, 'base64'),
        ephemeralPublicKey: Buffer.from(p.ephemeralPublicKey, 'base64')
      })),
      params: {
        finalPrice: payload.finalPrice
      }
    });
    
    console.log('⏳ Waiting for Arcium computation...');
    
    // Poll for results
    const result = await this.client.waitForResult(computation.id, {
      timeout: 60000, // 60 seconds
      interval: 2000  // Check every 2s
    });
    
    console.log('✅ Arcium computation complete!');
    
    // Parse winners from decrypted result
    return {
      jobId: computation.id,
      winners: result.outputs.map(output => ({
        commitment: output.commitment,
        payout: output.payout,
        recipient: output.recipient
      })),
      signature: result.signature
    };
  }
}

module.exports = ArciumComputeClient;
```

---

## Step 6: Test End-to-End

### Terminal 1: Backend
```bash
cd backend
npm install @arcium-hq/client @arcium-hq/reader
node server.js
```

### Terminal 2: Relayer
```bash
cd relayer
npm install @arcium-hq/client @arcium-hq/reader
node index.js
```

### Terminal 3: Frontend
```bash
cd frontend
npm install @arcium-hq/client
python3 -m http.server 8080
```

### Test Flow
1. Open `http://localhost:8080`
2. Submit encrypted prediction
3. Check backend logs → should show encrypted ciphertext
4. Wait for round to end
5. Relayer submits to Arcium → **real computation**
6. Winners settled on-chain

---

## ⚙️ Environment Variables

Update `.env` files:

```bash
# relayer/.env
ARCIUM_MODE=live  # Or 'mock' for testing

# backend/.env
ARCIUM_MXE_ID=<from-setup-script>
```

---

## 🔍 Verify It's Working

When relayer runs, you should see:
```
🔐 Submitting to real Arcium network...
⏳ Waiting for Arcium computation...
✅ Arcium computation complete!
   Job ID: arx-abc123...
   Winners: 3 predictions
```

If you see "Using MOCK" → SDK not configured, check installation.

---

## 📚 Resources

- **Arcium TypeScript SDK**: https://ts.arcium.com/api
- **Arcium Docs**: https://docs.arcium.com/developers
- **MXE Guide**: https://docs.arcium.com/developers/hello-world
- **Testnet Explorer**: https://explorer.arcium.com (if available)

---

## ❓ Troubleshooting

### "Cannot find module '@arcium-hq/client'"
→ Run `npm install @arcium-hq/client` in that directory

### "No MXEs found on testnet"
→ Testnet might be restarting. Use mock mode temporarily or check Arcium Discord

### "Computation timeout"
→ Increase timeout in `waitForResult()` or check Arcium network status

---

## 🎯 Summary

**What You Get**:
- ✅ Real encrypted predictions (not mock)
- ✅ Arcium testnet computation
- ✅ Privacy guarantees
- ✅ No node setup required
- ✅ Free testnet access

**What You DON'T Need**:
- ❌ Run your own Arcium node
- ❌ Complex server setup
- ❌ API keys or whitelisting
- ❌ Infrastructure costs

You're using Arcium as a **service** (like using an API), not running infrastructure!
