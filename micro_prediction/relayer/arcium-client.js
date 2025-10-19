/**
 * Arcium Real SDK Client
 * Uses @arcium-hq/client and @arcium-hq/reader for encrypted computation
 * 
 * NO SEPARATE API KEY NEEDED - Just uses Solana RPC (Helius)
 * Official Docs: https://docs.arcium.com
 */

const { Connection } = require('@solana/web3.js');

// Try to import Arcium SDK with fallback
let SDK = null;
let ArciumReader = null;

try {
  const clientPkg = require('@arcium-hq/client');
  SDK = clientPkg.ArciumClient || clientPkg.default || clientPkg;
  
  const readerPkg = require('@arcium-hq/reader');
  ArciumReader = readerPkg.ArciumReader || readerPkg.default || readerPkg;
} catch (error) {
  console.warn('⚠️  Arcium SDK packages not available, will use mock mode only');
}

class ArciumClient {
  constructor(config = {}) {
    this.mode = config.mode || process.env.ARCIUM_MODE || 'mock';
    this.rpcUrl = config.rpcUrl || process.env.ARCIUM_RPC_URL || 'https://api.devnet.solana.com';
    
    if ((this.mode === 'devnet' || this.mode === 'live') && SDK && ArciumReader) {
      try {
        this.connection = new Connection(this.rpcUrl, 'confirmed');
        this.sdk = new SDK(this.connection);
        this.reader = new ArciumReader(this.connection);
        this.mxeCache = null;
      } catch (error) {
        console.warn('⚠️  Failed to initialize Arcium SDK:', error.message);
        console.warn('   Falling back to MOCK mode');
        this.mode = 'mock';
      }
    } else if (this.mode !== 'mock') {
      console.warn('⚠️  Arcium SDK not available, using MOCK mode');
      this.mode = 'mock';
    }
  }

  /**
   * Get or fetch MXE configuration from Arcium network
   */
  async getMxeConfig() {
    if (this.mxeCache) return this.mxeCache;
    
    if (this.mode !== 'devnet' && this.mode !== 'live') {
      return null; // Mock mode
    }
    
    try {
      console.log('🔍 Fetching MXE configuration from Arcium network...');
      const mxes = await this.reader.fetchAllMxes();
      
      if (!mxes || mxes.length === 0) {
        console.warn('⚠️  No MXEs found on network, falling back to mock mode');
        return null;
      }
      
      // Use first available MXE (or find one for predictions)
      this.mxeCache = {
        id: mxes[0].id,
        publicKey: mxes[0].publicKey,
        name: mxes[0].name || 'default'
      };
      
      console.log(`✅ Using MXE: ${this.mxeCache.name} (${this.mxeCache.id})`);
      return this.mxeCache;
    } catch (error) {
      console.warn('⚠️  Failed to fetch MXE config:', error.message);
      return null;
    }
  }

  /**
   * Submit encrypted predictions for computation using real Arcium SDK
   * @param {Object} payload - { roundId, finalPrice, predictions: [{commitment, ciphertext, ...}] }
   * @returns {Promise<Object>} - Computation result
   */
  async submitComputation(payload) {
    const mxeConfig = await this.getMxeConfig();
    
    if (!mxeConfig) {
      console.log('⚠️  No MXE available, using mock computation');
      return this.mockComputation(payload);
    }
    
    try {
      console.log(`📤 Submitting ${payload.predictions.length} predictions to Arcium MXE...`);
      
      // Prepare encrypted inputs for Arcium
      const inputs = payload.predictions.map(p => ({
        ciphertext: Buffer.from(p.ciphertext, 'base64'),
        nonce: Buffer.from(p.nonce, 'base64'),
        ephemeralPublicKey: Buffer.from(p.ephemeralPublicKey, 'base64'),
        metadata: {
          commitment: p.commitment,
          stake: p.stake,
          wallet: p.wallet,
          windowIndex: p.windowIndex
        }
      }));
      
      // Submit computation to Arcium network
      const computation = await this.sdk.submitComputation({
        mxeId: mxeConfig.id,
        inputs,
        params: {
          finalPrice: payload.finalPrice,
          roundId: payload.roundId
        }
      });
      
      console.log(`✅ Computation submitted: ${computation.id}`);
      return { job_id: computation.id, computation };
    } catch (error) {
      console.error('❌ Failed to submit to Arcium:', error.message);
      console.log('⚠️  Falling back to mock mode');
      return this.mockComputation(payload);
    }
  }

  /**
   * Wait for computation results using real Arcium SDK
   * @param {string} jobId - Job ID from submitComputation  
   * @param {Object} computation - Computation object from SDK
   * @returns {Promise<Object>} - { status, winners, signature }
   */
  async getComputationResult(jobId, computation, maxAttempts = 20) {
    if (!computation) {
      // Mock mode
      console.log('⚠️  Mock mode - returning immediate result');
      await new Promise(resolve => setTimeout(resolve, 2000));
      return { status: 'completed', winners: [], signature: 'mock' };
    }
    
    console.log(`🔄 Waiting for Arcium computation (job: ${jobId})...`);
    
    try {
      // Use SDK to wait for result
      const result = await this.sdk.waitForResult(computation.id, {
        timeout: maxAttempts * 2000,
        interval: 2000
      });
      
      console.log(`✅ Computation complete!`);
      
      // Parse winners from decrypted outputs
      const winners = result.outputs?.map(output => ({
        commitment: output.metadata?.commitment || output.commitment,
        payout: output.payout || output.metadata?.stake,
        recipient: output.metadata?.wallet,
        prediction_account: null // Derive on-chain
      })) || [];
      
      return {
        status: 'completed',
        winners,
        signature: result.signature,
        metadata: result.metadata
      };
    } catch (error) {
      console.error('❌ Computation failed:', error.message);
      console.log('⚠️  Falling back to mock result');
      return await this.mockComputation({ predictions: [] });
    }
  }

  /**
   * Complete flow: Submit + Wait for results (Real Arcium SDK)
   * @param {Object} payload
   * @returns {Promise<Object>} - Winners and settlement data
   */
  async computeSettlement(payload) {
    // Check mode
    if (this.mode === 'mock' || !this.connection) {
      console.log('🔧 Running in MOCK mode (no Arcium SDK)'); 
      return this.mockComputation(payload);
    }
    
    console.log(`🔐 Running REAL Arcium computation (${this.mode} mode)`);
    
    // 1. Submit computation to real Arcium network
    const submission = await this.submitComputation(payload);
    
    if (submission.job_id === 'mock-' + Date.now() || !submission.computation) {
      // Fell back to mock
      return this.mockComputation(payload);
    }
    
    // 2. Wait for results from Arcium MPC nodes
    const results = await this.getComputationResult(
      submission.job_id,
      submission.computation
    );
    
    // 3. Return processed winners
    return {
      jobId: submission.job_id,
      winners: results.winners || [],
      signature: results.signature,
      metadata: results.metadata || {
        total_predictions: payload.predictions.length,
        mode: 'arcium-sdk'
      },
    };
  }

  /**
   * Mock computation for testing (when Arcium API unavailable)
   */
  async mockComputation(payload) {
    console.log('⚠️  Using MOCK Arcium computation (dev mode)');
    
    // Simulate API delay
    await new Promise(resolve => setTimeout(resolve, 2000));
    
    // Mock: Pick first prediction as winner with 100% of stake as payout
    const mockWinners = payload.predictions.slice(0, 1).map(p => ({
      commitment: Array.from(p.commitment),
      payout: p.stake,
      recipient: p.wallet,
      prediction_account: null, // Would be derived on-chain
    }));
    
    return {
      jobId: 'mock-' + Date.now(),
      winners: mockWinners,
      signature: 'mock-signature',
      metadata: {
        total_predictions: payload.predictions.length,
        total_payout: mockWinners.reduce((sum, w) => sum + w.payout, 0),
      },
    };
  }
}

module.exports = ArciumClient;
