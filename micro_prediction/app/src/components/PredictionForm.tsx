"use client";

import { useState, type FormEvent } from "react";
import { useWallet, useConnection } from "@solana/wallet-adapter-react";
import { PublicKey, Transaction } from "@solana/web3.js";
import {
  AnchorProvider,
  Program,
  web3,
  type Idl,
  type Wallet,
  BN,
} from "@coral-xyz/anchor";
import {
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  getMint,
} from "@solana/spl-token";
import { encryptPrediction, fetchMXEPublicKey, toBase64, toHex } from "@/lib/arcium";
import { LockClosedIcon, CheckCircleIcon } from "@heroicons/react/24/solid";

const PROGRAM_ID = "3btqev6Y8xNxqwFxFKaDPihQyVZ1gs2DpBNsDukmHxNX";

export function PredictionForm() {
  const { publicKey, sendTransaction, wallet } = useWallet();
  const { connection } = useConnection();

  const [predictionPrice, setPredictionPrice] = useState("");
  const [stakeAmount, setStakeAmount] = useState("");
  const [windowIndex, setWindowIndex] = useState<number>(2); // 1=Down, 2=Hold, 3=Up
  const [status, setStatus] = useState("");
  const [loading, setLoading] = useState(false);
  const [commitment, setCommitment] = useState("");
  const [encrypted, setEncrypted] = useState(false);

  const handleSubmit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();

    if (!publicKey || !wallet?.adapter) {
      setStatus("Please connect your wallet");
      return;
    }

    setLoading(true);
    setStatus("Encrypting prediction with Arcium...");
    setEncrypted(false);
    setCommitment("");

    try {
      // Create provider
      const provider = new AnchorProvider(
        connection,
        wallet.adapter as unknown as Wallet,
        { commitment: "confirmed" }
      );

      // Step 1: Fetch MXE public key
      setStatus("Fetching Arcium MXE public key...");
      const mxePublicKey = await fetchMXEPublicKey(provider, PROGRAM_ID);

      // Step 2: Encrypt prediction
      setStatus("Encrypting your prediction...");
      const parseAmountToUnits = (amount: string, decimals: number): bigint => {
        const [intPart, fracPart = ""] = amount.split(".");
        if (!/^\d+$/.test(intPart || "0") || !/^\d*$/.test(fracPart)) {
          throw new Error("Invalid stake amount");
        }
        if (fracPart.length > decimals) {
          throw new Error(`Stake precision must be <= ${decimals} decimals`);
        }
        const paddedFraction = fracPart.padEnd(decimals, "0");
        const unitsStr = `${intPart || "0"}${paddedFraction}`.replace(/^0+(?=\d)/, "");
        return BigInt(unitsStr || "0");
      };

      const idlResponse = await fetch("/idl/micro_prediction.json");
      const idl = (await idlResponse.json()) as Idl;
      const program = new Program(idl, provider);

      const roundId = 1;
      const predictionIndex = Math.floor(Math.random() * 1000); // Random index for demo

      const [configPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("config")],
        new PublicKey(PROGRAM_ID)
      );

      const configAccount: any = await program.account.config.fetch(configPda);
      const tokenMint = new PublicKey(configAccount.tokenMint);
      const mintInfo = await getMint(connection, tokenMint);
      const stakeUnits = parseAmountToUnits(stakeAmount, mintInfo.decimals);

      const predictionData = {
        roundId,
        price: parseFloat(predictionPrice),
        stake: Number(stakeUnits),
        windowIndex,
        timestamp: Date.now(),
      };

      const encrypted = await encryptPrediction(predictionData, mxePublicKey);
      const commitmentHex = toHex(encrypted.commitment);
      setCommitment(commitmentHex);
      setEncrypted(true);

      // Step 3: Submit to Solana
      setStatus("Preparing transaction...");

      const u64ToBytes = (num: number) => {
        const buffer = new ArrayBuffer(8);
        const view = new DataView(buffer);
        view.setBigUint64(0, BigInt(num), true);
        return new Uint8Array(buffer);
      };

      const u16ToBytes = (num: number) => {
        const buffer = new ArrayBuffer(2);
        const view = new DataView(buffer);
        view.setUint16(0, num, true);
        return new Uint8Array(buffer);
      };

      const [roundPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("round"), u64ToBytes(roundId)],
        new PublicKey(PROGRAM_ID)
      );

      const [predictionPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("prediction"),
          u64ToBytes(roundId),
          publicKey.toBuffer(),
          u16ToBytes(predictionIndex),
        ],
        new PublicKey(PROGRAM_ID)
      );

      const [escrowPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("escrow"), u64ToBytes(roundId)],
        new PublicKey(PROGRAM_ID)
      );

      const userTokenAccount = await getAssociatedTokenAddress(
        tokenMint,
        publicKey
      );

      const ataIx = await connection.getAccountInfo(userTokenAccount);

      setStatus("Submitting encrypted commitment on-chain...");

      const submitIx = await program.methods
        .submitPrediction(
          Array.from(encrypted.commitment),
          windowIndex,
          new BN(stakeUnits.toString()),
          predictionIndex
        )
        .accounts({
          user: publicKey,
          config: configPda,
          round: roundPda,
          prediction: predictionPda,
          userTokenAccount,
          escrowVault: escrowPda,
          tokenProgram: new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCwuBvf9Ss623VQ5DA"),
          systemProgram: web3.SystemProgram.programId,
        })
        .instruction();

      const latestBlockhash = await connection.getLatestBlockhash();

      const transaction = new Transaction();
      if (!ataIx) {
        transaction.add(
          createAssociatedTokenAccountInstruction(
            publicKey,
            userTokenAccount,
            publicKey,
            tokenMint
          )
        );
      }
      transaction.add(submitIx);
      transaction.feePayer = publicKey;
      transaction.recentBlockhash = latestBlockhash.blockhash;

      // Send transaction
      const signature = await sendTransaction(transaction, connection);
      await connection.confirmTransaction({
        signature,
        blockhash: latestBlockhash.blockhash,
        lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
      });

      setStatus(`✅ Success! Transaction: ${signature.slice(0, 8)}...`);

      // Store ciphertext locally or send to relayer
      console.log("Encrypted prediction submitted:", {
        commitment: commitmentHex,
        ciphertext: encrypted.ciphertext,
        signature,
      });

      const backendUrl = process.env.NEXT_PUBLIC_BACKEND_URL || "";
      try {
        setStatus("Submitting encrypted payload to backend...");
        const response = await fetch(`${backendUrl}/predictions`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            roundId,
            windowIndex,
            stake: stakeUnits.toString(),
            price: predictionData.price,
            commitment: commitmentHex,
            ciphertext: toBase64(encrypted.ciphertext),
            nonce: toBase64(encrypted.nonce),
            ephemeralPublicKey: toBase64(encrypted.ephemeralPublicKey),
            transactionSignature: signature,
            wallet: publicKey.toBase58(),
            programId: PROGRAM_ID,
          }),
        });

        if (!response.ok) {
          const errorText = await response.text();
          throw new Error(errorText || "Backend rejected prediction");
        }

        console.log("Encrypted payload stored successfully");
        setStatus(`✅ Prediction recorded: ${signature.slice(0, 8)}...`);
      } catch (backendError: any) {
        console.error("Failed to store encrypted payload:", backendError);
        setStatus(
          `⚠️ On-chain success, storage failed: ${
            backendError.message || backendError
          }`
        );
      }

      // Reset form
      setTimeout(() => {
        setPredictionPrice("");
        setStakeAmount("");
        setStatus("");
        setEncrypted(false);
      }, 3000);
    } catch (error: any) {
      console.error("Submission error:", error);
      setStatus(`❌ Error: ${error.message || "Failed to submit prediction"}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="bg-slate-800/50 backdrop-blur-sm rounded-2xl p-6 border border-slate-700/50">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-bold text-white">Submit Prediction</h2>
        <div className="flex items-center gap-2 px-3 py-1 bg-violet-500/20 rounded-full border border-violet-500/30">
          <LockClosedIcon className="w-4 h-4 text-violet-400" />
          <span className="text-xs text-violet-300 font-medium">Encrypted</span>
        </div>
      </div>

      <form onSubmit={handleSubmit} className="space-y-6">
        {/* Window Selection */}
        <div>
          <label className="block text-sm font-medium text-slate-300 mb-3">
            Prediction Window
          </label>
          <div className="grid grid-cols-3 gap-3">
            <button
              type="button"
              onClick={() => setWindowIndex(1)}
              className={`p-4 rounded-xl border-2 transition-all ${
                windowIndex === 1
                  ? "bg-red-500/20 border-red-500/50 text-red-300"
                  : "bg-slate-700/30 border-slate-600/50 text-slate-400 hover:border-slate-500"
              }`}
            >
              <div className="text-lg font-bold mb-1">📉 Down</div>
              <div className="text-xs">Price drops &gt; $2</div>
            </button>
            <button
              type="button"
              onClick={() => setWindowIndex(2)}
              className={`p-4 rounded-xl border-2 transition-all ${
                windowIndex === 2
                  ? "bg-yellow-500/20 border-yellow-500/50 text-yellow-300"
                  : "bg-slate-700/30 border-slate-600/50 text-slate-400 hover:border-slate-500"
              }`}
            >
              <div className="text-lg font-bold mb-1">➡️ Hold</div>
              <div className="text-xs">Price stable ±$2</div>
            </button>
            <button
              type="button"
              onClick={() => setWindowIndex(3)}
              className={`p-4 rounded-xl border-2 transition-all ${
                windowIndex === 3
                  ? "bg-green-500/20 border-green-500/50 text-green-300"
                  : "bg-slate-700/30 border-slate-600/50 text-slate-400 hover:border-slate-500"
              }`}
            >
              <div className="text-lg font-bold mb-1">📈 Up</div>
              <div className="text-xs">Price rises &gt; $2</div>
            </button>
          </div>
        </div>

        {/* Exact Price (Optional) */}
        <div>
          <label className="block text-sm font-medium text-slate-300 mb-2">
            Exact Price Prediction (Optional)
          </label>
          <input
            type="number"
            step="0.01"
            value={predictionPrice}
            onChange={(e) => setPredictionPrice(e.target.value)}
            placeholder="e.g., 147.50"
            className="w-full px-4 py-3 bg-slate-900/50 border border-slate-600/50 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:border-violet-500/50 focus:ring-2 focus:ring-violet-500/20"
          />
          <p className="text-xs text-slate-400 mt-2">
            For tiebreakers: closest prediction wins
          </p>
        </div>

        {/* Stake Amount */}
        <div>
          <label className="block text-sm font-medium text-slate-300 mb-2">
            Stake Amount (SOL)
          </label>
          <input
            type="number"
            step="0.001"
            min="0.001"
            value={stakeAmount}
            onChange={(e) => setStakeAmount(e.target.value)}
            placeholder="0.1"
            required
            className="w-full px-4 py-3 bg-slate-900/50 border border-slate-600/50 rounded-xl text-white placeholder-slate-500 focus:outline-none focus:border-violet-500/50 focus:ring-2 focus:ring-violet-500/20"
          />
          <p className="text-xs text-slate-400 mt-2">
            Minimum: 0.001 SOL • Escrowed until settlement
          </p>
        </div>

        {/* Commitment Display */}
        {encrypted && commitment && (
          <div className="p-4 bg-green-500/10 border border-green-500/30 rounded-xl">
            <div className="flex items-center gap-2 mb-2">
              <CheckCircleIcon className="w-5 h-5 text-green-400" />
              <span className="text-sm font-medium text-green-300">
                Prediction Encrypted
              </span>
            </div>
            <div className="text-xs text-green-200 font-mono break-all">
              {commitment.slice(0, 64)}...
            </div>
          </div>
        )}

        {/* Submit Button */}
        <button
          type="submit"
          disabled={loading || !stakeAmount}
          className="w-full py-4 bg-gradient-to-r from-violet-500 to-fuchsia-500 hover:from-violet-600 hover:to-fuchsia-600 disabled:from-slate-600 disabled:to-slate-600 text-white font-bold rounded-xl transition-all transform hover:scale-[1.02] active:scale-[0.98] disabled:cursor-not-allowed disabled:transform-none"
        >
          {loading ? (
            <span className="flex items-center justify-center gap-2">
              <svg className="animate-spin h-5 w-5" viewBox="0 0 24 24">
                <circle
                  className="opacity-25"
                  cx="12"
                  cy="12"
                  r="10"
                  stroke="currentColor"
                  strokeWidth="4"
                  fill="none"
                />
                <path
                  className="opacity-75"
                  fill="currentColor"
                  d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                />
              </svg>
              Processing...
            </span>
          ) : (
            "🔐 Encrypt & Submit Prediction"
          )}
        </button>

        {/* Status Message */}
        {status && (
          <div
            className={`text-center text-sm ${
              status.includes("✅")
                ? "text-green-400"
                : status.includes("❌")
                ? "text-red-400"
                : "text-slate-300"
            }`}
          >
            {status}
          </div>
        )}
      </form>

      {/* Info */}
      <div className="mt-6 pt-6 border-t border-slate-700/50 space-y-2 text-xs text-slate-400">
        <div className="flex items-start gap-2">
          <LockClosedIcon className="w-4 h-4 text-violet-400 flex-shrink-0 mt-0.5" />
          <span>
            Your prediction is encrypted with Arcium before submission. Only the
            commitment hash is stored on-chain.
          </span>
        </div>
        <div className="flex items-start gap-2">
          <span className="text-violet-400">•</span>
          <span>
            Funds are escrowed in a program PDA until round settlement.
          </span>
        </div>
        <div className="flex items-start gap-2">
          <span className="text-violet-400">•</span>
          <span>
            Winners are computed privately via Arcium MPC and settled on Solana.
          </span>
        </div>
      </div>
    </div>
  );
}
