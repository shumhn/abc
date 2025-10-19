"use client";

import { useEffect, useState } from "react";

const ROUND_DURATION = 180; // 3 minutes in seconds

export function RoundTimer() {
  const [timeLeft, setTimeLeft] = useState(ROUND_DURATION);
  const [roundNumber, setRoundNumber] = useState(1);
  const [totalPredictions, setTotalPredictions] = useState(8);
  const [totalStake, setTotalStake] = useState(1.2);

  useEffect(() => {
    const interval = setInterval(() => {
      setTimeLeft((prev) => {
        if (prev <= 1) {
          // Round ended, start new round
          setRoundNumber((r) => r + 1);
          setTotalPredictions(0);
          setTotalStake(0);
          return ROUND_DURATION;
        }
        return prev - 1;
      });
    }, 1000);

    return () => clearInterval(interval);
  }, []);

  const minutes = Math.floor(timeLeft / 60);
  const seconds = timeLeft % 60;
  const progress = ((ROUND_DURATION - timeLeft) / ROUND_DURATION) * 100;

  const getStatus = () => {
    if (timeLeft > 150) return { label: "Active", color: "green" };
    if (timeLeft > 30) return { label: "Active", color: "yellow" };
    if (timeLeft > 0) return { label: "Closing Soon", color: "orange" };
    return { label: "Settling", color: "blue" };
  };

  const status = getStatus();

  return (
    <div className="bg-slate-800/50 backdrop-blur-sm rounded-2xl p-6 border border-slate-700/50">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-slate-300">Round #{roundNumber}</h3>
        <span
          className={`text-xs px-3 py-1 rounded-full font-medium ${
            status.color === "green"
              ? "bg-green-500/20 text-green-400 border border-green-500/30"
              : status.color === "yellow"
              ? "bg-yellow-500/20 text-yellow-400 border border-yellow-500/30"
              : status.color === "orange"
              ? "bg-orange-500/20 text-orange-400 border border-orange-500/30"
              : "bg-blue-500/20 text-blue-400 border border-blue-500/30"
          }`}
        >
          {status.label}
        </span>
      </div>

      {/* Countdown */}
      <div className="text-center mb-4">
        <div className="text-5xl font-bold text-white tabular-nums">
          {minutes.toString().padStart(2, "0")}:{seconds.toString().padStart(2, "0")}
        </div>
        <div className="text-sm text-slate-400 mt-2">Time Remaining</div>
      </div>

      {/* Progress Bar */}
      <div className="relative h-2 bg-slate-700/50 rounded-full overflow-hidden mb-6">
        <div
          className="absolute top-0 left-0 h-full bg-gradient-to-r from-violet-500 to-fuchsia-500 transition-all duration-1000"
          style={{ width: `${progress}%` }}
        />
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 gap-4">
        <div className="text-center p-3 rounded-lg bg-slate-700/30">
          <div className="text-2xl font-bold text-violet-400">{totalPredictions}</div>
          <div className="text-xs text-slate-400 mt-1">Predictions</div>
        </div>
        <div className="text-center p-3 rounded-lg bg-slate-700/30">
          <div className="text-2xl font-bold text-fuchsia-400">{totalStake.toFixed(2)} SOL</div>
          <div className="text-xs text-slate-400 mt-1">Total Stake</div>
        </div>
      </div>

      {/* Info */}
      <div className="mt-6 pt-6 border-t border-slate-700/50 space-y-2 text-xs text-slate-400">
        <div className="flex justify-between">
          <span>Round Duration:</span>
          <span className="text-white font-medium">3 minutes</span>
        </div>
        <div className="flex justify-between">
          <span>Settlement:</span>
          <span className="text-white font-medium">Instant</span>
        </div>
      </div>
    </div>
  );
}
