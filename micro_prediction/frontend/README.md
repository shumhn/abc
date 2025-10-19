# 🎯 Prediction Market Frontend

A browser-based frontend for submitting encrypted predictions to the Solana prediction market program.

## ✅ Fixed Issues

### **Critical Browser Compatibility**
- ✅ Removed Node.js `Buffer` dependencies
- ✅ Added browser-compatible base64 encoding functions
- ✅ All cryptographic operations now work in browsers

### **Dependencies**
- ✅ Added missing `@noble/curves` and `@noble/hashes` packages
- ✅ Added Express for backend API
- ✅ Added Vite for modern bundling and dev server

### **Architecture**
- ✅ Created backend API server for Arcium MXE public key
- ✅ Set up Vite bundler with proper proxy configuration
- ✅ Modular client-side encryption implementation

## 🏗️ Architecture

```
┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│   Browser   │─────▶│  Vite Dev    │─────▶│  Backend    │
│  (Frontend) │      │   Server     │      │  API :3001  │
│   :3000     │      │  (Proxy)     │      └─────────────┘
└─────────────┘      └──────────────┘             │
      │                                            │
      │                                            ▼
      │                                   ┌─────────────┐
      └──────────────────────────────────▶│   Solana    │
                                          │  + Arcium   │
                                          └─────────────┘
```

## 📦 Installation

From the project root:

```bash
npm install
```

This installs all dependencies including:
- `@noble/curves` - Elliptic curve cryptography
- `@noble/hashes` - SHA-256 hashing
- `express` - Backend API server
- `vite` - Modern frontend bundler

## 🚀 Running the Application

### **Development Mode** (Recommended)

Start both frontend and backend together:

```bash
npm run dev
```

This starts:
- **Frontend**: http://localhost:3000 (Vite dev server)
- **Backend**: http://localhost:3001 (Express API)

### **Separate Servers**

Start them individually:

```bash
# Terminal 1 - Backend API
npm run backend

# Terminal 2 - Frontend
npm run frontend
```

## 🔐 How It Works

### **1. Encryption Flow**

```javascript
// 1. Get MXE public key from backend
const mxePubkey = await getMXEPublicKey(rpcUrl);

// 2. Generate ephemeral keypair
const privateKey = x25519.utils.randomPrivateKey();
const publicKey = x25519.getPublicKey(privateKey);

// 3. Derive shared secret
const sharedSecret = x25519.scalarMult(privateKey, mxePubkey);

// 4. Encrypt prediction data
const encrypted = await crypto.subtle.encrypt(
  { name: "AES-GCM", iv: nonce },
  key,
  payload
);

// 5. Create commitment hash
const commitment = sha256(ciphertext + publicKey + nonce);
```

### **2. Submission Flow**

1. User fills out prediction form
2. Frontend encrypts prediction using Arcium MXE public key
3. Creates cryptographic commitment
4. Displays encrypted data to user
5. **TODO**: Submit commitment to Solana (requires wallet integration)

## 📂 File Structure

```
frontend/
├── index.html              # Main UI
├── predictionClient.js     # Encryption & API client
└── README.md              # This file

backend/
└── server.js              # Express API for MXE public key

vite.config.js             # Vite bundler configuration
```

## 🔧 Configuration

### **Environment Variables**

Create `.env` in project root (if not exists):

```bash
ANCHOR_PROVIDER_URL=http://127.0.0.1:8899
BACKEND_PORT=3001
```

### **Vite Proxy**

The Vite dev server proxies `/arcium/*` requests to the backend API:

```javascript
// vite.config.js
proxy: {
  '/arcium': {
    target: 'http://localhost:3001',
    changeOrigin: true
  }
}
```

## 🛠️ API Endpoints

### **Backend API**

#### `GET /arcium/public-key?rpc=<RPC_URL>`

Fetches the MXE (Multi-Party Execution) public key from Arcium.

**Response:**
```json
{
  "mxePubkey": "base64_encoded_public_key",
  "cached": false
}
```

#### `GET /health`

Health check endpoint.

**Response:**
```json
{
  "status": "ok",
  "programId": "3btqev6Y8xNxqwFxFKaDPihQyVZ1gs2DpBNsDukmHxNX"
}
```

## ⚠️ Known Limitations

### **Wallet Integration Required**

The current implementation creates encrypted commitments but **does not submit them on-chain**. 

To complete the flow, you need to:

1. **Add Solana Wallet Adapter**
   ```bash
   npm install @solana/wallet-adapter-react \
               @solana/wallet-adapter-react-ui \
               @solana/wallet-adapter-wallets
   ```

2. **Load Program IDL**
   - Build the Solana program: `anchor build`
   - IDL will be at: `target/idl/micro_prediction.json`

3. **Implement Transaction Submission**
   - Connect to user wallet
   - Create transaction using Anchor
   - Sign and submit with user's wallet

See `predictionClient.js` line 77+ for implementation notes.

## 🎨 UI Features

- **Real-time encryption** - Instant client-side encryption
- **Commitment display** - Shows encrypted data and commitment hash
- **Status updates** - Live feedback during encryption and submission
- **Modern design** - Glassmorphism styling with gradients
- **Responsive** - Works on all screen sizes

## 🔍 Testing

### **Test Encryption Flow**

1. Start the servers: `npm run dev`
2. Open http://localhost:3000
3. Fill out the form:
   - **RPC URL**: `http://127.0.0.1:8899` (or your local validator)
   - **Round ID**: `0`
   - **Predicted Price**: `50000` (example)
   - **Stake**: `1000000` (1M lamports = 0.001 SOL)
4. Click "Encrypt & Submit"
5. Check browser console for detailed logs
6. View encrypted output on screen

### **Expected Console Output**

```
Fetching MXE public key from program: 3btqev6Y8xNxqwFxFKaDPihQyVZ1gs2DpBNsDukmHxNX
MXE public key fetched successfully
Sending commitment to Solana {"rpcUrl":"...","roundId":0,...}
⚠️  Wallet integration required to submit on-chain transaction
📝 Commitment ready for submission: {...}
```

## 📚 Next Steps

### **For Full Integration**

1. **Deploy the Solana Program**
   ```bash
   anchor build
   anchor deploy
   ```

2. **Initialize Program State**
   - Run initialization scripts
   - Create rounds
   - Set up Pyth oracle integration

3. **Add Wallet Support**
   - Integrate Solana Wallet Adapter
   - Update `sendCommitment()` function
   - Handle transaction signing

4. **Production Deployment**
   - Build for production: `npm run build`
   - Deploy to hosting (Vercel, Netlify, etc.)
   - Update RPC URLs to devnet/mainnet

## 🐛 Troubleshooting

### **Backend fails to start**

```bash
Error: Cannot find module '@arcium-hq/client'
```

**Solution**: Run `npm install` from project root.

### **MXE public key not found**

```bash
Error: MXE public key not found
```

**Solution**: Make sure the Solana program is deployed and the MXE account is initialized.

### **CORS errors**

The backend includes CORS headers. If you still see CORS errors, check:
- Backend is running on port 3001
- Vite proxy is configured correctly
- No firewall blocking localhost connections

### **Module not found in browser**

If you see `Cannot find module '@noble/curves'` in browser:

**Solution**: Make sure you're accessing via Vite dev server (http://localhost:3000), not opening the HTML file directly.

## 📖 References

- **Arcium Documentation**: https://docs.arcium.com/
- **Pyth Network**: https://pyth.network/
- **Solana Web3.js**: https://solana-labs.github.io/solana-web3.js/
- **Anchor Framework**: https://www.anchor-lang.com/

---

**Built with ❤️ for encrypted on-chain predictions**
