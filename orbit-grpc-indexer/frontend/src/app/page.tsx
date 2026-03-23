"use client";

import { useEffect, useState } from "react";
import StatsCard from "@/components/StatsCard";
import LiveFeed from "@/components/LiveFeed";
import { fetchHealth, type HealthData } from "@/lib/api";

export default function Dashboard() {
  const [health, setHealth] = useState<HealthData | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    let active = true;

    async function poll() {
      try {
        const data = await fetchHealth();
        if (active) {
          setHealth(data);
          setError(false);
        }
      } catch {
        if (active) setError(true);
      }
    }

    poll();
    const interval = setInterval(poll, 5000);
    return () => {
      active = false;
      clearInterval(interval);
    };
  }, []);

  const healthy = health?.healthy ?? false;
  const jetstreamUp = health?.jetstream_connected ?? false;
  const yellowstoneUp = health?.yellowstone_connected ?? false;

  let sourceBreakdown = "N/A";
  if (health) {
    const parts: string[] = [];
    if (jetstreamUp) parts.push("Jetstream");
    if (yellowstoneUp) parts.push("Yellowstone");
    sourceBreakdown = parts.length > 0 ? parts.join(" + ") : "None";
  }

  return (
    <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
      <div className="mb-8 flex items-center gap-4">
        <div className="flex items-center gap-3">
          <div
            className={`h-3 w-3 rounded-full ${
              error
                ? "bg-orbit-error shadow-[0_0_12px_rgba(248,113,113,0.6)]"
                : healthy
                  ? "bg-orbit-success shadow-[0_0_12px_rgba(74,222,128,0.6)] animate-pulse"
                  : "bg-orbit-yellow shadow-[0_0_12px_rgba(251,191,36,0.6)]"
            }`}
          />
          <h1 className="text-2xl font-bold text-orbit-text">
            Indexer Dashboard
          </h1>
        </div>
        <span className="text-sm text-orbit-muted">
          {error
            ? "API unreachable"
            : healthy
              ? "All systems operational"
              : "Degraded"}
        </span>
      </div>

      <div className="mb-8 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <StatsCard
          label="Transactions Indexed"
          value={health ? health.transactions_indexed.toLocaleString() : "--"}
          accent
        />
        <StatsCard
          label="Last Indexed Slot"
          value={health ? health.last_indexed_slot.toLocaleString() : "--"}
        />
        <StatsCard
          label="Active Sources"
          value={sourceBreakdown}
          sub={
            health
              ? `Jetstream: ${jetstreamUp ? "UP" : "DOWN"} / Yellowstone: ${yellowstoneUp ? "UP" : "DOWN"}`
              : undefined
          }
        />
      </div>

      <LiveFeed />
    </div>
  );
}
