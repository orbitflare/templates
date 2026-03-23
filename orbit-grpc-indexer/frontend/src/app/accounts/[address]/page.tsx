"use client";

import { useEffect, useState, useCallback, use } from "react";
import Link from "next/link";
import TransactionRow from "@/components/TransactionRow";
import Pagination from "@/components/Pagination";
import { fetchAccountTransactions, type Transaction } from "@/lib/api";

export default function AccountPage({
  params,
}: {
  params: Promise<{ address: string }>;
}) {
  const { address } = use(params);
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [initialLoad, setInitialLoad] = useState(true);

  const loadMore = useCallback(
    async (append = false) => {
      setLoading(true);
      try {
        const res = await fetchAccountTransactions(address, {
          limit: 25,
          cursor: append ? (cursor ?? undefined) : undefined,
        });
        if (append) {
          setTransactions((prev) => [...prev, ...res.data]);
        } else {
          setTransactions(res.data);
        }
        setCursor(res.pagination.next_cursor);
        setHasMore(res.pagination.has_more);
      } catch {
        // silently fail
      } finally {
        setLoading(false);
        setInitialLoad(false);
      }
    },
    [address, cursor]
  );

  useEffect(() => {
    loadMore(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [address]);

  return (
    <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
      <Link
        href="/transactions"
        className="mb-6 inline-flex items-center gap-2 text-sm text-orbit-muted transition-colors hover:text-orbit-teal"
      >
        <span>&larr;</span> Back to Transactions
      </Link>

      <div className="mb-6">
        <h1 className="text-xl font-bold text-orbit-text">Account</h1>
        <p className="mt-2 break-all font-mono text-sm text-orbit-teal">
          {address}
        </p>
      </div>

      <div className="overflow-hidden rounded-xl border border-orbit-border bg-orbit-surface">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="border-b border-orbit-border bg-orbit-bg/50">
                <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                  Signature
                </th>
                <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                  Slot
                </th>
                <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                  Source
                </th>
                <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                  Status
                </th>
                <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                  Instructions
                </th>
                <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                  CPI
                </th>
                <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-orbit-muted">
                  Indexed At
                </th>
              </tr>
            </thead>
            <tbody>
              {initialLoad ? (
                <tr>
                  <td
                    colSpan={7}
                    className="px-4 py-16 text-center text-sm text-orbit-muted"
                  >
                    Loading transactions...
                  </td>
                </tr>
              ) : transactions.length === 0 ? (
                <tr>
                  <td
                    colSpan={7}
                    className="px-4 py-16 text-center text-sm text-orbit-muted"
                  >
                    No transactions found for this account
                  </td>
                </tr>
              ) : (
                transactions.map((tx) => (
                  <TransactionRow key={tx.signature} tx={tx} />
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      <Pagination
        hasMore={hasMore}
        loading={loading}
        onLoadMore={() => loadMore(true)}
      />
    </div>
  );
}
