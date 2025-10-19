"use client";

import { useEffect, useState } from "react";
import { PriceDisplay } from "@/components/PriceDisplay";
import { RoundTimer } from "@/components/RoundTimer";
import { PredictionForm } from "@/components/PredictionForm";
import { useWallet } from "@solana/wallet-adapter-react";
import { WalletMultiButton } from "@solana/wallet-adapter-react-ui";

export default function Home() {
  const { connected } = useWallet();
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) return null;

  return (
    <main className="min-h-screen p-4 md:p-8">
      <div className="max-w-7xl mx-auto">
        {/* Header */}
        <header className="mb-8 flex items-center justify-between">
          <div>
            <h1 className="text-3xl md:text-4xl font-bold bg-gradient-to-r from-violet-400 to-fuchsia-400 bg-clip-text text-transparent">
              Encrypted Prediction Market
            </h1>
            <p className="text-slate-400 mt-1">
              Powered by Arcium • Pyth • Solana
            </p>
          </div>
          <WalletMultiButton />
        </header>

        {/* Stats Bar */}
        <div className="grid grid-cols-3 gap-4 mb-8">
          <div className="bg-slate-800/50 backdrop-blur-sm rounded-xl p-4 border border-slate-700/50">
            <div className="text-sm text-slate-400">Active Players</div>
            <div className="text-2xl font-bold text-violet-400">12</div>
          </div>
          <div className="bg-slate-800/50 backdrop-blur-sm rounded-xl p-4 border border-slate-700/50">
            <div className="text-sm text-slate-400">Total Volume</div>
            <div className="text-2xl font-bold text-fuchsia-400">2.4 SOL</div>
          </div>
          <div className="bg-slate-800/50 backdrop-blur-sm rounded-xl p-4 border border-slate-700/50">
            <div className="text-sm text-slate-400">Avg Latency</div>
            <div className="text-2xl font-bold text-cyan-400">45ms</div>
          </div>
        </div>

        {/* Main Grid */}
        <div className="grid lg:grid-cols-3 gap-6">
          {/* Left: Price + Timer */}
          <div className="lg:col-span-1 space-y-6">
            <PriceDisplay />
            <RoundTimer />
          </div>

          {/* Center: Prediction Form */}
          <div className="lg:col-span-2">
            {connected ? (
              <PredictionForm />
            ) : (
              <div className="bg-slate-800/50 backdrop-blur-sm rounded-2xl p-12 border border-slate-700/50 text-center">
                <div className="max-w-md mx-auto">
                  <h3 className="text-2xl font-bold mb-4">
                    Connect Your Wallet
                  </h3>
                  <p className="text-slate-400 mb-6">
                    Connect your Solana wallet to start making encrypted predictions
                  </p>
                  <WalletMultiButton className="!mx-auto" />
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Technology Badges */}
        <div className="mt-8 flex flex-wrap gap-3 justify-center">
          <div className="px-4 py-2 bg-violet-500/10 border border-violet-500/20 rounded-full text-sm text-violet-300">
            🔐 Arcium Encrypted
          </div>
          <div className="px-4 py-2 bg-cyan-500/10 border border-cyan-500/20 rounded-full text-sm text-cyan-300">
            ⚡ MagicBlock Ephemeral
          </div>
          <div className="px-4 py-2 bg-fuchsia-500/10 border border-fuchsia-500/20 rounded-full text-sm text-fuchsia-300">
            📊 Pyth Oracle
          </div>
        </div>
      </div>
    </main>
  );
}
