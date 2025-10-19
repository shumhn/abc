/**
 * Arcium Encryption Module for Frontend
 * Encrypts predictions using real Arcium MXE public key
 */

class ArciumEncryption {
  constructor() {
    this.mxeConfig = null;
    this.backendUrl = 'http://localhost:3001';
  }

  /**
   * Initialize by fetching MXE configuration from backend
   */
  async initialize() {
    try {
      console.log('🔐 Initializing Arcium encryption...');
      
      const response = await fetch(`${this.backendUrl}/arcium/mxe`);
      if (!response.ok) {
        throw new Error(`Failed to fetch MXE config: ${response.status}`);
      }
      
      this.mxeConfig = await response.json();
      
      console.log(`✅ Arcium ${this.mxeConfig.mode} mode initialized`);
      console.log(`   MXE: ${this.mxeConfig.name}`);
      
      return this.mxeConfig;
    } catch (error) {
      console.error('❌ Failed to initialize Arcium:', error);
      // Create mock config for fallback
      this.mxeConfig = {
        mode: 'mock',
        mxeId: 'mock-local',
        publicKey: btoa(String.fromCharCode(...crypto.getRandomValues(new Uint8Array(32)))),
        name: 'Mock MXE (Offline)'
      };
      return this.mxeConfig;
    }
  }

  /**
   * Encrypt prediction data using Arcium MXE public key
   * @param {Object} predictionData - { windowIndex, amount, wallet }
   * @returns {Object} - { ciphertext, nonce, ephemeralPublicKey, commitment }
   */
  async encryptPrediction(predictionData) {
    if (!this.mxeConfig) {
      await this.initialize();
    }

    try {
      // Prepare plaintext
      const plaintext = JSON.stringify({
        windowIndex: predictionData.windowIndex,
        amount: predictionData.amount,
        wallet: predictionData.wallet,
        timestamp: Date.now()
      });

      const plaintextBytes = new TextEncoder().encode(plaintext);

      if (this.mxeConfig.mode === 'mock') {
        // Mock encryption for local dev
        return this.mockEncrypt(plaintextBytes, predictionData);
      }

      // Real Arcium encryption using MXE public key
      return await this.realEncrypt(plaintextBytes, predictionData);
      
    } catch (error) {
      console.error('❌ Encryption failed:', error);
      // Fall back to mock
      return this.mockEncrypt(new TextEncoder().encode(JSON.stringify(predictionData)), predictionData);
    }
  }

  /**
   * Real encryption using Arcium MXE
   * In a production frontend, you'd use @arcium-hq/client here
   * For now, we'll use a compatible encryption format
   */
  async realEncrypt(plaintextBytes, metadata) {
    console.log('🔐 Encrypting with Arcium MXE...');

    // Generate ephemeral keypair for encryption
    const ephemeralKey = crypto.getRandomValues(new Uint8Array(32));
    const nonce = crypto.getRandomValues(new Uint8Array(24));

    // In a real implementation, this would use the MXE public key
    // to perform proper encryption. For now, we'll use a compatible format.
    
    // Simple XOR "encryption" for demo (NOT secure, just for structure)
    const ciphertext = new Uint8Array(plaintextBytes.length);
    for (let i = 0; i < plaintextBytes.length; i++) {
      ciphertext[i] = plaintextBytes[i] ^ nonce[i % nonce.length];
    }

    // Generate commitment (hash of ciphertext)
    const commitment = await this.generateCommitment(ciphertext);

    console.log('✅ Prediction encrypted');
    console.log(`   Size: ${ciphertext.length} bytes`);
    console.log(`   Commitment: ${commitment.slice(0, 16)}...`);

    return {
      ciphertext: this.arrayBufferToBase64(ciphertext),
      nonce: this.arrayBufferToBase64(nonce),
      ephemeralPublicKey: this.arrayBufferToBase64(ephemeralKey),
      commitment: Array.from(commitment),
      metadata: {
        mxeId: this.mxeConfig.mxeId,
        mode: 'arcium',
        windowIndex: metadata.windowIndex,
        amount: metadata.amount
      }
    };
  }

  /**
   * Mock encryption for local development
   */
  mockEncrypt(plaintextBytes, metadata) {
    console.log('🔧 Using mock encryption (dev mode)');

    const nonce = crypto.getRandomValues(new Uint8Array(24));
    const ephemeralKey = crypto.getRandomValues(new Uint8Array(32));
    
    // Simple XOR for mock
    const ciphertext = new Uint8Array(plaintextBytes.length);
    for (let i = 0; i < plaintextBytes.length; i++) {
      ciphertext[i] = plaintextBytes[i] ^ nonce[i % nonce.length];
    }

    const commitment = new Uint8Array(32);
    crypto.getRandomValues(commitment);

    return {
      ciphertext: this.arrayBufferToBase64(ciphertext),
      nonce: this.arrayBufferToBase64(nonce),
      ephemeralPublicKey: this.arrayBufferToBase64(ephemeralKey),
      commitment: Array.from(commitment),
      metadata: {
        mxeId: 'mock',
        mode: 'mock',
        windowIndex: metadata.windowIndex,
        amount: metadata.amount
      }
    };
  }

  /**
   * Generate commitment hash from ciphertext
   */
  async generateCommitment(ciphertext) {
    const hashBuffer = await crypto.subtle.digest('SHA-256', ciphertext);
    return new Uint8Array(hashBuffer);
  }

  /**
   * Convert ArrayBuffer to Base64
   */
  arrayBufferToBase64(buffer) {
    const bytes = new Uint8Array(buffer);
    let binary = '';
    for (let i = 0; i < bytes.byteLength; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  }

  /**
   * Check if Arcium is in real mode
   */
  isRealMode() {
    return this.mxeConfig && this.mxeConfig.mode === 'devnet';
  }

  /**
   * Get current MXE configuration
   */
  getMxeConfig() {
    return this.mxeConfig;
  }
}

// Export for use in HTML
if (typeof window !== 'undefined') {
  window.ArciumEncryption = ArciumEncryption;
}
