// Browser-compatible base64 helpers
function base64ToUint8Array(base64) {
  const binaryString = atob(base64);
  const len = binaryString.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes;
}

function uint8ArrayToBase64(bytes) {
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function uint8ArrayToHex(bytes) {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export async function getMXEPublicKey(rpcUrl) {
  const response = await fetch(
    "/arcium/public-key?rpc=" + encodeURIComponent(rpcUrl)
  );
  if (!response.ok) {
    throw new Error("Failed to fetch MXE public key");
  }
  const { mxePubkey } = await response.json();
  return base64ToUint8Array(mxePubkey);
}

export async function encryptPayload({
  roundId,
  prediction,
  stake,
  mxePubkey,
}) {
  const { x25519 } = await import("@noble/curves/ed25519");
  const { sha256 } = await import("@noble/hashes/sha256");

  const privateKey = x25519.utils.randomPrivateKey();
  const publicKey = x25519.getPublicKey(privateKey);
  const sharedSecret = x25519.scalarMult(privateKey, mxePubkey);

  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const payload = new TextEncoder().encode(
    JSON.stringify({ roundId, prediction, stake, nonce: Array.from(nonce) })
  );
  const key = await crypto.subtle.importKey(
    "raw",
    sharedSecret,
    { name: "AES-GCM" },
    false,
    ["encrypt"]
  );

  const ciphertextBuffer = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: nonce },
    key,
    payload
  );

  const ciphertext = new Uint8Array(ciphertextBuffer);
  const commitment = sha256
    .create()
    .update(ciphertext)
    .update(publicKey)
    .update(nonce)
    .digest();

  return {
    ciphertext: uint8ArrayToBase64(ciphertext),
    commitment: uint8ArrayToHex(commitment),
    nonce: uint8ArrayToBase64(nonce),
    ephPublicKey: uint8ArrayToBase64(publicKey),
  };
}

export async function sendCommitment({ rpcUrl, roundId, stake, commitment }) {
  console.log(
    "Sending commitment to Solana",
    JSON.stringify({ rpcUrl, roundId, stake, commitment })
  );

  // Note: This requires wallet connection to work properly
  // For now, we're showing the data that would be submitted

  // TODO: Implement full wallet integration
  // The submission requires:
  // 1. Connected Solana wallet (Phantom, Solflare, etc.)
  // 2. Program IDL loaded
  // 3. Token account for paying stake
  // 4. Signature from user's wallet

  // Example structure (not yet implemented):
  // const { Connection, PublicKey } = await import('@solana/web3.js');
  // const { AnchorProvider, Program } = await import('@coral-xyz/anchor');
  //
  // Submit transaction using:
  // - program.methods.submitPrediction(commitment, windowIndex, stake, predictionIndex)
  // - accounts: { config, round, prediction, userTokenAccount, escrowVault, etc. }

  console.warn(
    "⚠️  Wallet integration required to submit on-chain transaction"
  );
  console.log("📝 Commitment ready for submission:", {
    commitment,
    roundId,
    stake: `${stake} lamports`,
    status: "pending_wallet_connection",
  });

  await new Promise((resolve) => setTimeout(resolve, 500));

  return {
    success: false,
    message:
      "Wallet connection required. Please integrate with Solana wallet adapter.",
    commitment,
    roundId,
    stake,
  };
}
