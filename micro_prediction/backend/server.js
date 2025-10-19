const express = require("express");
const { PublicKey, Connection } = require("@solana/web3.js");
const anchor = require("@coral-xyz/anchor");
require('dotenv').config();
const {
  PythHttpClient,
  getPythProgramKeyForCluster,
} = require("@pythnetwork/client");
const crypto = require("crypto");

// Try to import Arcium SDK (may not be available or have different exports)
let ArciumReader = null;
try {
  const arciumPkg = require("@arcium-hq/reader");
  ArciumReader = arciumPkg.ArciumReader || arciumPkg.default || arciumPkg;
} catch (error) {
  console.warn("⚠️  Arcium SDK not available, will use mock mode");
}

const app = express();
const PORT = process.env.BACKEND_PORT || 3001;
const DEFAULT_RPC = process.env.ANCHOR_PROVIDER_URL || "http://127.0.0.1:8899";
const SOL_FEED_ID =
  process.env.SOL_PRICE_FEED || "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix";
const predictionStore = new Map();

// Enable CORS
app.use(express.json({ limit: "1mb" }));
app.use((req, res, next) => {
  res.header("Access-Control-Allow-Origin", "*");
  res.header(
    "Access-Control-Allow-Headers",
    "Origin, X-Control-Requested-With, Content-Type, Accept"
  );
  next();
});

// Get program ID from the workspace
let programId;
try {
  const idl = require("../target/idl/micro_prediction.json");
  programId = new PublicKey(idl.address || idl.metadata?.address);
} catch (err) {
  console.warn("Could not load program ID from IDL:", err.message);
  // Fallback - you'll need to replace this with your actual program ID
  programId = null;
}

// Cache for MXE configuration
let cachedMXEConfig = null;
let cacheTimestamp = 0;
const CACHE_TTL = 300000; // 5 minutes

// Initialize Arcium on startup
let arciumReader = null;
const arciumMode = process.env.ARCIUM_MODE || 'mock';
const arciumRpcUrl = process.env.ARCIUM_RPC_URL || 'https://api.devnet.solana.com';

if ((arciumMode === 'devnet' || arciumMode === 'live') && ArciumReader) {
  try {
    const connection = new Connection(arciumRpcUrl, 'confirmed');
    arciumReader = new ArciumReader(connection);
    console.log(`✅ Arcium Reader initialized (${arciumMode} mode)`);
    console.log(`   RPC: ${arciumRpcUrl.substring(0, 50)}...`);
  } catch (error) {
    console.warn('⚠️  Failed to initialize Arcium Reader:', error.message);
    console.warn('   Falling back to MOCK mode');
  }
} else if (arciumMode !== 'mock') {
  console.warn('⚠️  Arcium SDK not available, using MOCK mode');
}

app.get("/arcium/mxe", async (req, res) => {
  try {
    // Check if we have a valid cached config
    const now = Date.now();
    if (cachedMXEConfig && now - cacheTimestamp < CACHE_TTL) {
      return res.json({ ...cachedMXEConfig, cached: true });
    }

    if (!arciumReader) {
      // Mock mode - return fake MXE for local testing
      console.log('⚠️  Running in MOCK mode - returning fake MXE config');
      return res.json({
        mode: 'mock',
        mxeId: 'mock-mxe-id',
        publicKey: Buffer.from(crypto.randomBytes(32)).toString('base64'),
        name: 'Mock MXE (Local Dev)',
        cached: false
      });
    }

    console.log('🔍 Fetching MXE configuration from Arcium network...');

    // Fetch all MXEs from Arcium network
    const mxes = await arciumReader.fetchAllMxes();
    
    if (!mxes || mxes.length === 0) {
      console.warn('⚠️  No MXEs found on Arcium network');
      // Fall back to mock
      return res.json({
        mode: 'mock',
        mxeId: 'mock-mxe-id',
        publicKey: Buffer.from(crypto.randomBytes(32)).toString('base64'),
        name: 'Mock MXE (No Network MXEs)',
        cached: false
      });
    }

    // Use first available MXE (or could filter for specific use case)
    const mxe = mxes[0];
    
    const config = {
      mode: arciumMode,
      mxeId: mxe.id.toString(),
      publicKey: mxe.publicKey.toString(),
      name: mxe.name || 'Arcium MXE',
      cached: false
    };

    // Cache the result
    cachedMXEConfig = config;
    cacheTimestamp = now;

    console.log(`✅ MXE config fetched: ${config.name} (${config.mxeId})`);

    res.json(config);
  } catch (error) {
    console.error("❌ Error fetching MXE config:", error);
    
    // Fall back to mock on error
    res.json({
      mode: 'mock',
      mxeId: 'mock-mxe-id',
      publicKey: Buffer.from(crypto.randomBytes(32)).toString('base64'),
      name: 'Mock MXE (Error Fallback)',
      error: error.message,
      cached: false
    });
  }
});

// Legacy endpoint for backward compatibility
app.get("/arcium/public-key", async (req, res) => {
  try {
    const mxeConfig = await fetch('http://localhost:' + PORT + '/arcium/mxe').then(r => r.json());
    res.json({
      mxePubkey: mxeConfig.publicKey,
      cached: mxeConfig.cached
    });
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

app.get("/pyth/price", async (req, res) => {
  const feed = req.query.feed || SOL_FEED_ID;
  const rpcUrl = req.query.rpc || DEFAULT_RPC;

  try {
    const connection = new Connection(rpcUrl, "confirmed");
    const pythClient = new PythHttpClient(
      connection,
      getPythProgramKeyForCluster("devnet")
    );
    const data = await pythClient.getData();

    const priceInfo = data.productPrice.get(feed);
    if (!priceInfo?.price) {
      throw new Error("Price feed unavailable");
    }

    res.json({
      feed,
      price: priceInfo.price,
      confidence: priceInfo.confidence,
      publishTime: priceInfo.publishTime,
    });
  } catch (error) {
    console.error("Error fetching Pyth price:", error);
    res.status(500).json({
      error: error.message || "Failed to fetch Pyth price",
    });
  }
});

const assertString = (value, name) => {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${name} must be a non-empty string`);
  }
  return value.trim();
};

app.post("/predictions", (req, res) => {
  try {
    const {
      roundId,
      windowIndex,
      stake,
      price,
      commitment,
      ciphertext,
      nonce,
      ephemeralPublicKey,
      transactionSignature,
      wallet,
      programId,
    } = req.body || {};

    if (typeof roundId !== "number" || roundId < 0) {
      throw new Error("roundId must be a non-negative number");
    }
    if (typeof windowIndex !== "number" || windowIndex < 1 || windowIndex > 3) {
      throw new Error("windowIndex must be between 1 and 3");
    }
    assertString(stake, "stake");
    assertString(commitment, "commitment");
    assertString(ciphertext, "ciphertext");
    assertString(nonce, "nonce");
    assertString(ephemeralPublicKey, "ephemeralPublicKey");
    assertString(transactionSignature, "transactionSignature");
    assertString(wallet, "wallet");
    assertString(programId, "programId");
    if (price !== null && price !== undefined && typeof price !== "number") {
      throw new Error("price must be a number when provided");
    }

    const entry = {
      id: crypto.randomUUID(),
      roundId,
      windowIndex,
      stake,
      price: price ?? null,
      commitment,
      ciphertext,
      nonce,
      ephemeralPublicKey,
      transactionSignature,
      wallet,
      programId,
      receivedAt: Date.now(),
    };

    if (!predictionStore.has(roundId)) {
      predictionStore.set(roundId, []);
    }
    predictionStore.get(roundId).push(entry);

    res.status(201).json({ success: true, id: entry.id });
  } catch (error) {
    res.status(400).json({ error: error.message || "Invalid payload" });
  }
});

// POST endpoint for Arcium-encrypted predictions
app.post("/predictions/:roundId", (req, res) => {
  try {
    const roundId = Number(req.params.roundId);
    if (Number.isNaN(roundId) || roundId < 0) {
      return res
        .status(400)
        .json({ error: "roundId must be a non-negative number" });
    }

    const {
      commitment,
      ciphertext,
      nonce,
      ephemeralPublicKey,
      wallet,
      stake,
      windowIndex,
      transactionSignature,
    } = req.body || {};

    // Flexible validation - allow different formats
    if (!commitment || !Array.isArray(commitment)) {
      return res.status(400).json({ error: "commitment must be an array" });
    }
    if (!ciphertext || typeof ciphertext !== "string") {
      return res.status(400).json({ error: "ciphertext must be a string" });
    }
    if (!wallet || typeof wallet !== "string") {
      return res.status(400).json({ error: "wallet must be a string" });
    }

    const entry = {
      id: crypto.randomUUID(),
      roundId,
      commitment,
      ciphertext,
      nonce: nonce || "",
      ephemeralPublicKey: ephemeralPublicKey || "",
      wallet,
      stake: stake || 0,
      windowIndex: windowIndex !== undefined ? windowIndex : null,
      transactionSignature: transactionSignature || "pending",
      receivedAt: new Date().toISOString(),
    };

    if (!predictionStore.has(roundId)) {
      predictionStore.set(roundId, []);
    }
    predictionStore.get(roundId).push(entry);

    console.log(`✅ Prediction stored for round ${roundId}:`, {
      id: entry.id,
      wallet: entry.wallet.substring(0, 10) + "...",
      stake: entry.stake,
      ciphertextSize: entry.ciphertext.length,
    });

    res.status(201).json({
      success: true,
      id: entry.id,
      roundId,
      message: "Encrypted prediction stored successfully",
    });
  } catch (error) {
    console.error("Error storing prediction:", error);
    res.status(500).json({ error: error.message || "Failed to store prediction" });
  }
});

app.get("/predictions/:roundId", (req, res) => {
  const roundId = Number(req.params.roundId);
  if (Number.isNaN(roundId) || roundId < 0) {
    return res
      .status(400)
      .json({ error: "roundId must be a non-negative number" });
  }
  const entries = predictionStore.get(roundId) || [];
  res.json({ roundId, predictions: entries });
});

app.delete("/predictions/:roundId", (req, res) => {
  const roundId = Number(req.params.roundId);
  if (Number.isNaN(roundId) || roundId < 0) {
    return res
      .status(400)
      .json({ error: "roundId must be a non-negative number" });
  }

  const existed = predictionStore.delete(roundId);
  res.json({ roundId, cleared: existed });
});

app.get("/health", (req, res) => {
  res.json({
    status: "ok",
    programId: programId?.toString() || "not configured",
  });
});

app.listen(PORT, () => {
  console.log(`Backend API server listening on port ${PORT}`);
  console.log(`Program ID: ${programId?.toString() || "not configured"}`);
  console.log(`Health check: http://localhost:${PORT}/health`);
});
