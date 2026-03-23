"use client";

import { useEffect, useState, use } from "react";
import Link from "next/link";
import SourceBadge from "@/components/SourceBadge";
import { fetchTransaction, type TransactionDetail } from "@/lib/api";

function truncateAddr(addr: string): string {
  if (addr.length <= 16) return addr;
  return `${addr.slice(0, 6)}...${addr.slice(-6)}`;
}

export default function TransactionDetailPage({
  params,
}: {
  params: Promise<{ signature: string }>;
}) {
  const { signature } = use(params);
  const [detail, setDetail] = useState<TransactionDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [logsOpen, setLogsOpen] = useState(false);

  useEffect(() => {
    let active = true;
    fetchTransaction(signature)
      .then((res) => {
        if (active) setDetail(res.data);
      })
      .catch((err) => {
        if (active) setError(err.message);
      });
    return () => {
      active = false;
    };
  }, [signature]);

  if (error) {
    return (
      <div className="mx-auto max-w-5xl px-4 py-8 sm:px-6 lg:px-8">
        <Link
          href="/transactions"
          className="mb-6 inline-flex items-center gap-2 text-sm text-orbit-muted transition-colors hover:text-orbit-teal"
        >
          <span>&larr;</span> Back to Transactions
        </Link>
        <div className="rounded-xl border border-orbit-error/30 bg-orbit-error/5 p-8 text-center">
          <p className="text-orbit-error">{error}</p>
        </div>
      </div>
    );
  }

  if (!detail) {
    return (
      <div className="mx-auto max-w-5xl px-4 py-8 sm:px-6 lg:px-8">
        <Link
          href="/transactions"
          className="mb-6 inline-flex items-center gap-2 text-sm text-orbit-muted transition-colors hover:text-orbit-teal"
        >
          <span>&larr;</span> Back to Transactions
        </Link>
        <div className="rounded-xl border border-orbit-border bg-orbit-surface p-16 text-center text-orbit-muted">
          Loading transaction...
        </div>
      </div>
    );
  }

  const tx = detail.transaction;
  const accounts = tx.accounts?.length > 0 ? tx.accounts : tx.account_keys || [];

  return (
    <div className="mx-auto max-w-5xl px-4 py-8 sm:px-6 lg:px-8">
      <Link
        href="/transactions"
        className="mb-6 inline-flex items-center gap-2 text-sm text-orbit-muted transition-colors hover:text-orbit-teal"
      >
        <span>&larr;</span> Back to Transactions
      </Link>

      <div className="mb-6 flex items-center gap-4">
        <h1 className="text-xl font-bold text-orbit-text">
          Transaction Details
        </h1>
        {tx.success ? (
          <span className="inline-flex items-center gap-1.5 rounded-md border border-orbit-success/30 bg-orbit-success/10 px-2.5 py-1 text-xs font-medium text-orbit-success">
            <span className="h-1.5 w-1.5 rounded-full bg-orbit-success" />
            Success
          </span>
        ) : (
          <span className="inline-flex items-center gap-1.5 rounded-md border border-orbit-error/30 bg-orbit-error/10 px-2.5 py-1 text-xs font-medium text-orbit-error">
            <span className="h-1.5 w-1.5 rounded-full bg-orbit-error" />
            Failed
          </span>
        )}
      </div>

      <div className="space-y-6">
        <div className="rounded-xl border border-orbit-border bg-orbit-surface p-6">
          <h2 className="mb-4 text-sm font-semibold uppercase tracking-wider text-orbit-muted">
            Overview
          </h2>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <p className="text-xs text-orbit-muted">Signature</p>
              <p className="mt-1 break-all font-mono text-sm text-orbit-teal">
                {tx.signature}
              </p>
            </div>
            <div>
              <p className="text-xs text-orbit-muted">Slot</p>
              <p className="mt-1 font-mono text-sm text-orbit-text">
                {tx.slot.toLocaleString()}
              </p>
            </div>
            <div>
              <p className="text-xs text-orbit-muted">Source</p>
              <div className="mt-1">
                <SourceBadge source={tx.source} />
              </div>
            </div>
            <div>
              <p className="text-xs text-orbit-muted">Fee</p>
              <p className="mt-1 font-mono text-sm text-orbit-text">
                {tx.fee != null ? `${tx.fee.toLocaleString()} lamports` : "N/A"}
              </p>
            </div>
            <div>
              <p className="text-xs text-orbit-muted">Instructions</p>
              <p className="mt-1 font-mono text-sm text-orbit-text">
                {tx.num_instructions ?? "N/A"}
              </p>
            </div>
            <div>
              <p className="text-xs text-orbit-muted">CPI Data</p>
              <p className="mt-1 text-sm">
                {tx.has_cpi_data ? (
                  <span className="text-orbit-mint">Yes</span>
                ) : (
                  <span className="text-orbit-muted">No</span>
                )}
              </p>
            </div>
            <div>
              <p className="text-xs text-orbit-muted">Indexed At</p>
              <p className="mt-1 text-sm text-orbit-text">
                {new Date(tx.indexed_at).toLocaleString()}
              </p>
            </div>
            {tx.enriched_at && (
              <div>
                <p className="text-xs text-orbit-muted">Enriched At</p>
                <p className="mt-1 text-sm text-orbit-text">
                  {new Date(tx.enriched_at).toLocaleString()}
                </p>
              </div>
            )}
          </div>
        </div>

        {accounts.length > 0 && (
          <div className="rounded-xl border border-orbit-border bg-orbit-surface p-6">
            <h2 className="mb-4 text-sm font-semibold uppercase tracking-wider text-orbit-muted">
              Account Keys ({accounts.length})
            </h2>
            <div className="space-y-2">
              {accounts.map((key: string, i: number) => (
                <div
                  key={i}
                  className="flex items-center justify-between rounded-lg bg-orbit-bg/50 px-4 py-2.5"
                >
                  <div className="flex items-center gap-3">
                    <span className="flex h-6 w-6 items-center justify-center rounded bg-orbit-border font-mono text-xs text-orbit-muted">
                      {i}
                    </span>
                    <Link
                      href={`/accounts/${key}`}
                      className="font-mono text-sm text-orbit-teal transition-colors hover:text-orbit-mint"
                    >
                      <span className="hidden sm:inline">{key}</span>
                      <span className="sm:hidden">{truncateAddr(key)}</span>
                    </Link>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {tx.log_messages && tx.log_messages.length > 0 && (
          <div className="rounded-xl border border-orbit-border bg-orbit-surface">
            <button
              onClick={() => setLogsOpen(!logsOpen)}
              className="flex w-full items-center justify-between p-6 text-left transition-colors hover:bg-orbit-bg/30"
            >
              <h2 className="text-sm font-semibold uppercase tracking-wider text-orbit-muted">
                Log Messages
                <span className="ml-2 text-orbit-teal">
                  ({tx.log_messages.length})
                </span>
              </h2>
              <span
                className={`text-orbit-muted transition-transform duration-200 ${
                  logsOpen ? "rotate-180" : ""
                }`}
              >
                &#9660;
              </span>
            </button>
            {logsOpen && (
              <div className="border-t border-orbit-border p-6">
                <div className="max-h-96 overflow-y-auto rounded-lg bg-orbit-bg p-4">
                  {tx.log_messages.map((msg: string, i: number) => (
                    <p
                      key={i}
                      className="font-mono text-xs leading-6 text-orbit-text/80"
                    >
                      <span className="mr-3 inline-block w-8 text-right text-orbit-muted">
                        {i + 1}
                      </span>
                      {msg}
                    </p>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {detail.inner_instructions && detail.inner_instructions.length > 0 && (
          <div className="rounded-xl border border-orbit-border bg-orbit-surface p-6">
            <h2 className="mb-4 text-sm font-semibold uppercase tracking-wider text-orbit-muted">
              Inner Instructions / CPIs ({detail.inner_instructions.length})
            </h2>
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  <tr className="border-b border-orbit-border">
                    <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                      Idx
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                      Depth
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                      Program ID
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                      Accounts
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                      Data
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {detail.inner_instructions.map((ix) => (
                    <tr
                      key={ix.id}
                      className="border-b border-orbit-border/50 transition-colors hover:bg-orbit-teal/5"
                    >
                      <td className="whitespace-nowrap px-4 py-3 font-mono text-sm text-orbit-muted">
                        {ix.instruction_idx}
                      </td>
                      <td className="whitespace-nowrap px-4 py-3 font-mono text-sm text-orbit-muted">
                        {ix.depth}
                      </td>
                      <td className="whitespace-nowrap px-4 py-3">
                        <Link
                          href={`/accounts/${ix.program_id}`}
                          className="font-mono text-sm text-orbit-teal transition-colors hover:text-orbit-mint"
                        >
                          {truncateAddr(ix.program_id)}
                        </Link>
                      </td>
                      <td className="whitespace-nowrap px-4 py-3 font-mono text-xs text-orbit-muted">
                        {ix.accounts?.length || 0} accounts
                      </td>
                      <td className="max-w-xs truncate px-4 py-3 font-mono text-xs text-orbit-muted">
                        {ix.data
                          ? ix.data.length > 40
                            ? `${ix.data.slice(0, 40)}...`
                            : ix.data
                          : "N/A"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
