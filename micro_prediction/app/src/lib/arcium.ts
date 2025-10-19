import { x25519 } from "@noble/curves/ed25519";
import { sha256 } from "@noble/hashes/sha256";
import { getMXEPublicKey } from "@arcium-hq/client";
import { AnchorProvider } from "@coral-xyz/anchor";

export interface PredictionData {
  roundId: number;
  price: number;
  stake: number;
  windowIndex: number;
  timestamp: number;
}

export interface EncryptedPrediction {
  ciphertext: Uint8Array;
  commitment: Uint8Array;
  nonce: Uint8Array;
  ephemeralPublicKey: Uint8Array;
}

/**
 * Encrypt prediction data using Arcium MXE public key
 */
export async function encryptPrediction(
  predictionData: PredictionData,
  mxePublicKey: Uint8Array
): Promise<EncryptedPrediction> {
  // Generate ephemeral x25519 keypair
  const ephemeralPrivateKey = x25519.utils.randomPrivateKey();
  const ephemeralPublicKey = x25519.getPublicKey(ephemeralPrivateKey);

  // Derive shared secret
  const sharedSecret = x25519.scalarMult(ephemeralPrivateKey, mxePublicKey);

  // Generate random nonce for AES-GCM
  const nonce = crypto.getRandomValues(new Uint8Array(12));

  // Prepare payload
  const payload = new TextEncoder().encode(
    JSON.stringify({
      roundId: predictionData.roundId,
      price: predictionData.price,
      stake: predictionData.stake,
      windowIndex: predictionData.windowIndex,
      timestamp: predictionData.timestamp,
      nonce: Array.from(nonce),
    })
  );

  // Import key for AES-GCM
  const key = await crypto.subtle.importKey(
    "raw",
    sharedSecret,
    { name: "AES-GCM" },
    false,
    ["encrypt"]
  );

  // Encrypt the payload
  const ciphertextBuffer = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: nonce },
    key,
    payload
  );

  const ciphertext = new Uint8Array(ciphertextBuffer);

  // Compute commitment = sha256(ciphertext || ephemeralPublicKey || nonce)
  const commitment = sha256
    .create()
    .update(ciphertext)
    .update(ephemeralPublicKey)
    .update(nonce)
    .digest();

  return {
    ciphertext,
    commitment,
    nonce,
    ephemeralPublicKey,
  };
}

/**
 * Fetch MXE public key from Arcium
 */
export async function fetchMXEPublicKey(
  provider: AnchorProvider,
  programId: string
): Promise<Uint8Array> {
  try {
    const mxePublicKey = await getMXEPublicKey(
      provider,
      new (
        await import("@solana/web3.js")
      ).PublicKey(programId)
    );
    return mxePublicKey;
  } catch (error) {
    console.error("Failed to fetch MXE public key:", error);
    throw new Error("Could not fetch Arcium MXE public key");
  }
}

/**
 * Convert Uint8Array to hex string
 */
export function toHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/**
 * Convert Uint8Array to base64
 */
export function toBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}
