"use client";

import Link from "next/link";
import SourceBadge from "./SourceBadge";
import { useLiveTransactions } from "@/lib/ws";

function truncateSig(sig: string): string {
  if (sig.length <= 16) return sig;
  return `${sig.slice(0, 8)}...${sig.slice(-8)}`;
}

export default function LiveFeed() {
  const { transactions, connected } = useLiveTransactions(30);

  return (
    <div className="rounded-xl border border-orbit-border bg-orbit-surface">
      <div className="flex items-center justify-between border-b border-orbit-border px-5 py-4">
        <div className="flex items-center gap-3">
          <h2 className="text-sm font-semibold text-orbit-text">
            Live Transaction Feed
          </h2>
          <div className="flex items-center gap-1.5">
            <span
              className={`h-2 w-2 rounded-full ${
                connected
                  ? "bg-orbit-success animate-pulse shadow-[0_0_8px_rgba(74,222,128,0.6)]"
                  : "bg-orbit-error"
              }`}
            />
            <span className="text-xs text-orbit-muted">
              {connected ? "Connected" : "Disconnected"}
            </span>
          </div>
        </div>
        <span className="rounded-md bg-orbit-bg px-2 py-1 font-mono text-xs text-orbit-muted">
          WS
        </span>
      </div>
      <div className="h-[400px] overflow-y-auto">
        {transactions.length === 0 ? (
          <div className="flex h-full items-center justify-center text-sm text-orbit-muted">
            {connected
              ? "Waiting for transactions..."
              : "Connecting to WebSocket..."}
          </div>
        ) : (
          <div className="divide-y divide-orbit-border/40">
            {transactions.map((tx, i) => (
              <div
                key={`${tx.signature}-${i}`}
                className="flex items-center justify-between px-5 py-3 transition-all duration-300 animate-in fade-in slide-in-from-top-2"
                style={{ animationDelay: `${i * 20}ms` }}
              >
                <div className="flex items-center gap-4">
                  <Link
                    href={`/transactions/${tx.signature}`}
                    className="font-mono text-sm text-orbit-teal transition-colors hover:text-orbit-mint"
                  >
                    {truncateSig(tx.signature)}
                  </Link>
                  <SourceBadge source={tx.source} />
                </div>
                <div className="flex items-center gap-4">
                  <span className="font-mono text-xs text-orbit-muted">
                    Slot {tx.slot.toLocaleString()}
                  </span>
                  {tx.success ? (
                    <span className="h-2 w-2 rounded-full bg-orbit-success shadow-[0_0_6px_rgba(74,222,128,0.4)]" />
                  ) : (
                    <span className="h-2 w-2 rounded-full bg-orbit-error shadow-[0_0_6px_rgba(248,113,113,0.4)]" />
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
