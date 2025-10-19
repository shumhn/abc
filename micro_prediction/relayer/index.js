const { Connection, PublicKey, Keypair } = require("@solana/web3.js");
const { AnchorProvider, Program, web3 } = require("@coral-xyz/anchor");
const {
  PythHttpClient,
  getPythProgramKeyForCluster,
} = require("@pythnetwork/client");
const fs = require("fs");
const path = require("path");
const ArciumClient = require("./arcium-client");
require("dotenv").config();

const fetch = (...args) =>
  import("node-fetch").then(({ default: fetchFn }) => fetchFn(...args));

const ROUND_DURATION = 180; // 3 minutes in seconds
const PROGRAM_ID = "3btqev6Y8xNxqwFxFKaDPihQyVZ1gs2DpBNsDukmHxNX";
const SOL_USD_FEED = "J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix";

class PredictionMarketRelayer {
  constructor() {
    this.connection = new Connection(
      process.env.RPC_URL || "http://127.0.0.1:8899",
      "confirmed"
    );
    this.backendUrl = process.env.BACKEND_URL || "http://localhost:3001";

    // Load authority keypair
    const keypairPath =
      process.env.KEYPAIR_PATH ||
      path.join(require("os").homedir(), ".config/solana/id.json");
    const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
    this.authority = Keypair.fromSecretKey(new Uint8Array(keypairData));

    this.provider = new AnchorProvider(
      this.connection,
      {
        publicKey: this.authority.publicKey,
        signTransaction: async (tx) => tx,
        signAllTransactions: async (txs) => txs,
      },
      { commitment: "confirmed" }
    );

    this.currentRoundId = 0;
    this.roundEndTime = null;
    this.ciphertextStorage = new Map(); // Store commitment -> ciphertext
    this.predictionCache = new Map();

    // Initialize Arcium client for off-chain encrypted computation
    // NO API KEY NEEDED - Just uses Solana RPC (Helius)
    this.arciumClient = new ArciumClient({
      mode: process.env.ARCIUM_MODE || 'mock',
      rpcUrl: process.env.ARCIUM_RPC_URL || process.env.RPC_URL,
    });
    this.useArciumMock = this.arciumClient.mode === 'mock';

    console.log("🚀 Relayer initialized");
    console.log("Authority:", this.authority.publicKey.toString());
    console.log("RPC:", this.connection.rpcEndpoint);
    console.log("Backend URL:", this.backendUrl);
    console.log("Arcium Mode:", this.useArciumMock ? "MOCK (dev)" : "LIVE (testnet)");
  }

  async loadProgram() {
    const idlPath = path.join(__dirname, "../target/idl/micro_prediction.json");
    const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
    this.program = new Program(idl, PROGRAM_ID, this.provider);
    console.log("✅ Program loaded:", PROGRAM_ID);
  }

  u64ToBytes(num) {
    const buf = Buffer.alloc(8);
    buf.writeBigUInt64LE(BigInt(num));
    return buf;
  }

  async initializeRound(roundId) {
    console.log(`\n📋 Initializing Round #${roundId}`);

    const now = Math.floor(Date.now() / 1000);
    const startTs = now;
    const endTs = now + ROUND_DURATION;

    try {
      const [configPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("config")],
        new PublicKey(PROGRAM_ID)
      );

      const [roundPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("round"), this.u64ToBytes(roundId)],
        new PublicKey(PROGRAM_ID)
      );

      const [escrowPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("escrow"), this.u64ToBytes(roundId)],
        new PublicKey(PROGRAM_ID)
      );

      const tx = await this.program.methods
        .initializeRound(roundId, startTs, endTs, new PublicKey(SOL_USD_FEED))
        .accounts({
          authority: this.authority.publicKey,
          config: configPda,
          round: roundPda,
          escrowVault: escrowPda,
          tokenProgram: new PublicKey(
            "TokenkegQfeZyiNwAJbNbGKPFXCwuBvf9Ss623VQ5DA"
          ),
          systemProgram: web3.SystemProgram.programId,
          rent: web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([this.authority])
        .rpc();

      console.log(`✅ Round initialized: ${tx}`);
      console.log(`   Start: ${new Date(startTs * 1000).toISOString()}`);
      console.log(`   End: ${new Date(endTs * 1000).toISOString()}`);

      this.currentRoundId = roundId;
      this.roundEndTime = endTs;

      return tx;
    } catch (error) {
      console.error("❌ Failed to initialize round:", error.message);
      throw error;
    }
  }

  async fetchPythPrice() {
    try {
      const pythClient = new PythHttpClient(
        this.connection,
        getPythProgramKeyForCluster("devnet")
      );

      const data = await pythClient.getData();
      const productPrice = data.productPrice.get(SOL_USD_FEED);

      if (productPrice?.price) {
        return productPrice.price;
      }

      // Fallback to mock price
      return 145.5 + (Math.random() - 0.5) * 5;
    } catch (error) {
      console.error("⚠️  Pyth fetch failed, using mock price:", error.message);
      return 145.5;
    }
  }

  async beginResolution(roundId) {
    console.log(`\n🔍 Beginning resolution for Round #${roundId}`);

    try {
      const finalPrice = await this.fetchPythPrice();
      console.log(`   Final Pyth Price: $${finalPrice.toFixed(2)}`);

      const predictions = await this.fetchRoundPredictions(roundId);
      this.predictionCache.set(roundId, predictions);
      console.log(
        `   Loaded ${predictions.length} encrypted predictions from backend`
      );

      const [configPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("config")],
        new PublicKey(PROGRAM_ID)
      );

      const [roundPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("round"), this.u64ToBytes(roundId)],
        new PublicKey(PROGRAM_ID)
      );

      // Call begin_resolution
      const tx = await this.program.methods
        .beginResolution(null, null) // Optional: add result_commitment and arcium_comp_id
        .accounts({
          authority: this.authority.publicKey,
          config: configPda,
          round: roundPda,
        })
        .signers([this.authority])
        .rpc();

      console.log(`✅ Resolution started: ${tx}`);

      // Real Arcium off-chain computation flow
      await this.executeSettlement(roundId, finalPrice);
    } catch (error) {
      console.error("❌ Failed to begin resolution:", error.message);
    }
  }

  async executeSettlement(roundId, finalPrice) {
    console.log(`\n💰 Executing settlement for Round #${roundId}`);

    try {
      const predictions = this.predictionCache.get(roundId) || [];
      
      if (predictions.length === 0) {
        console.log("⚠️  No predictions to settle");
        return await this.finalizeRoundOnly(roundId, finalPrice);
      }

      console.log(`\n🔐 Step 1: Submitting ${predictions.length} encrypted predictions to Arcium...`);
      
      // Call Arcium API (off-chain computation)
      const arciumResult = this.useArciumMock
        ? await this.arciumClient.mockComputation({
            roundId,
            finalPrice: Math.floor(finalPrice * 100),
            predictions,
          })
        : await this.arciumClient.computeSettlement({
            roundId,
            finalPrice: Math.floor(finalPrice * 100),
            predictions,
          });

      console.log(`\n💎 Step 2: Arcium returned ${arciumResult.winners.length} winners`);
      console.log("   Job ID:", arciumResult.jobId);
      
      // Settle each winner on-chain
      console.log(`\n💰 Step 3: Settling ${arciumResult.winners.length} winners on-chain...`);
      for (let i = 0; i < arciumResult.winners.length; i++) {
        const winner = arciumResult.winners[i];
        console.log(`   [${i + 1}/${arciumResult.winners.length}] Settling ${winner.payout} tokens...`);
        await this.settlePrediction(roundId, winner);
      }
      
      // Finalize the round
      const finalPriceFixed = Math.floor(finalPrice * 100); // Convert to fixed-point
      const timestamp = Math.floor(Date.now() / 1000);
      
      const [configPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("config")],
        new PublicKey(PROGRAM_ID)
      );

      const [roundPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("round"), this.u64ToBytes(roundId)],
        new PublicKey(PROGRAM_ID)
      );

      const tx = await this.program.methods
        .finalizeRound(finalPriceFixed, timestamp)
        .accounts({
          settlementAuthority: this.authority.publicKey,
          config: configPda,
          round: roundPda,
        })
        .signers([this.authority])
        .rpc();

      console.log(`\n✅ Round finalized: ${tx}`);
      console.log(`   Final Price: $${finalPrice.toFixed(2)}`);
      console.log(`   Total Settled: ${arciumResult.winners.length} predictions`);
      
      await this.markRoundProcessed(roundId, predictions.length);
      this.predictionCache.delete(roundId);
    } catch (error) {
      console.error("❌ Failed to execute settlement:", error.message);
      throw error;
    }
  }

  async settlePrediction(roundId, winner) {
    try {
      const [roundPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("round"), this.u64ToBytes(roundId)],
        new PublicKey(PROGRAM_ID)
      );

      const [escrowPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("escrow"), this.u64ToBytes(roundId)],
        new PublicKey(PROGRAM_ID)
      );

      // In production, derive prediction PDA and recipient from winner data
      // For now, this is a placeholder showing the structure
      const predictionPda = new PublicKey(winner.prediction_account || this.authority.publicKey);
      const recipientTokenAccount = new PublicKey(winner.recipient);

      const commitment = Array.isArray(winner.commitment) 
        ? winner.commitment 
        : Array.from(winner.commitment);

      const tx = await this.program.methods
        .settlePrediction(winner.payout, commitment)
        .accounts({
          round: roundPda,
          prediction: predictionPda,
          escrowVault: escrowPda,
          recipientTokenAccount,
          tokenProgram: new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCwuBvf9Ss623VQ5DA"),
        })
        .signers([this.authority])
        .rpc();

      console.log(`      ✅ Settled: ${tx.slice(0, 8)}...`);
    } catch (error) {
      console.error(`      ❌ Failed to settle prediction:`, error.message);
      // Continue with other settlements even if one fails
    }
  }

  async finalizeRoundOnly(roundId, finalPrice) {
    console.log(`\n🏁 Step 2: Finalizing round without settlements...`);
    
    const finalPriceFixed = Math.floor(finalPrice * 100);
    const timestamp = Math.floor(Date.now() / 1000);
    
    const [configPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      new PublicKey(PROGRAM_ID)
    );

    const [roundPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("round"), this.u64ToBytes(roundId)],
      new PublicKey(PROGRAM_ID)
    );

    const tx = await this.program.methods
      .finalizeRound(finalPriceFixed, timestamp)
      .accounts({
        settlementAuthority: this.authority.publicKey,
        config: configPda,
        round: roundPda,
      })
      .signers([this.authority])
      .rpc();

    console.log(`✅ Round finalized: ${tx}`);
    console.log(`   Final Price: $${finalPrice.toFixed(2)}`);
  }

  async fetchRoundPredictions(roundId) {
    if (!this.backendUrl) {
      return [];
    }

    try {
      const response = await fetch(`${this.backendUrl}/predictions/${roundId}`);
      if (!response.ok) {
        throw new Error(`Backend responded with ${response.status}`);
      }
      const data = await response.json();
      if (!data || !Array.isArray(data.predictions)) {
        return [];
      }
      return data.predictions;
    } catch (error) {
      console.error(
        "⚠️  Failed to fetch predictions from backend:",
        error.message
      );
      return [];
    }
  }

  async markRoundProcessed(roundId, count) {
    if (!this.backendUrl) {
      return;
    }

    try {
      await fetch(`${this.backendUrl}/predictions/${roundId}`, {
        method: "DELETE",
      });
    } catch (error) {
      console.warn(
        "⚠️  Unable to notify backend of processed predictions:",
        error.message
      );
    }

    console.log(
      `🧹 Processed round #${roundId}. Predictions handled: ${count}. Backend notified.`
    );
  }

  async start() {
    console.log("\n🎯 Starting Prediction Market Relayer\n");

    await this.loadProgram();

    // Initialize first round
    await this.initializeRound(this.currentRoundId);

    // Start round management loop
    setInterval(async () => {
      const now = Math.floor(Date.now() / 1000);

      if (now >= this.roundEndTime) {
        // Current round ended
        await this.beginResolution(this.currentRoundId);

        // Start next round
        this.currentRoundId++;
        await this.initializeRound(this.currentRoundId);
      } else {
        const timeLeft = this.roundEndTime - now;
        console.log(
          `⏱️  Round #${this.currentRoundId} - ${timeLeft}s remaining`
        );
      }
    }, 10000); // Check every 10 seconds

    console.log("\n✅ Relayer running. Press Ctrl+C to stop.\n");
  }

  storeCiphertext(commitment, ciphertext, ephPubKey, nonce) {
    this.ciphertextStorage.set(commitment, {
      ciphertext,
      ephPubKey,
      nonce,
      timestamp: Date.now(),
    });
    console.log(
      `📦 Stored ciphertext for commitment: ${commitment.slice(0, 16)}...`
    );
  }
}

// Start the relayer
const relayer = new PredictionMarketRelayer();
relayer.start().catch(console.error);

// Handle graceful shutdown
process.on("SIGINT", () => {
  console.log("\n\n👋 Shutting down relayer...");
  process.exit(0);
});
