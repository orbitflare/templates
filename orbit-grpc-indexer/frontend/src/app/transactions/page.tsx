"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import TransactionRow from "@/components/TransactionRow";
import Pagination from "@/components/Pagination";
import SourceBadge from "@/components/SourceBadge";
import {
  fetchTransactions,
  type Transaction,
  type TransactionFilters,
} from "@/lib/api";

export default function TransactionsPage() {
  const router = useRouter();
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [initialLoad, setInitialLoad] = useState(true);

  const [searchSig, setSearchSig] = useState("");
  const [sourceFilter, setSourceFilter] = useState("");
  const [successFilter, setSuccessFilter] = useState("");
  const [slotMin, setSlotMin] = useState("");
  const [slotMax, setSlotMax] = useState("");

  const buildFilters = useCallback(
    (nextCursor?: string): TransactionFilters => {
      const filters: TransactionFilters = { limit: 25 };
      if (nextCursor) filters.cursor = nextCursor;
      if (successFilter === "true") filters.success = true;
      if (successFilter === "false") filters.success = false;
      if (slotMin) filters.slot_min = Number(slotMin);
      if (slotMax) filters.slot_max = Number(slotMax);
      if (sourceFilter) filters.source = sourceFilter;
      return filters;
    },
    [successFilter, slotMin, slotMax, sourceFilter]
  );

  const loadTransactions = useCallback(
    async (append = false) => {
      setLoading(true);
      try {
        const filters = buildFilters(append ? (cursor ?? undefined) : undefined);
        const res = await fetchTransactions(filters);
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
    [buildFilters, cursor]
  );

  useEffect(() => {
    loadTransactions(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [successFilter, slotMin, slotMax, sourceFilter]);

  function handleSearch(e: React.FormEvent) {
    e.preventDefault();
    const query = searchSig.trim().toLowerCase();
    if (!query) return;
    const match = transactions.find((tx) =>
      tx.signature.toLowerCase().startsWith(query)
    );
    if (match) {
      router.push(`/transactions/${match.signature}`);
    }
  }

  const displayedTransactions = searchSig.trim()
    ? transactions.filter((tx) =>
        tx.signature.toLowerCase().startsWith(searchSig.trim().toLowerCase())
      )
    : transactions;

  return (
    <div className="mx-auto max-w-7xl px-4 py-8 sm:px-6 lg:px-8">
      <h1 className="mb-6 text-2xl font-bold text-orbit-text">
        Transactions
      </h1>

      <form onSubmit={handleSearch} className="mb-6">
        <div className="flex gap-3">
          <input
            type="text"
            value={searchSig}
            onChange={(e) => setSearchSig(e.target.value)}
            placeholder="Search by transaction signature..."
            className="flex-1 rounded-lg border border-orbit-border bg-orbit-surface px-4 py-2.5 font-mono text-sm text-orbit-text placeholder:text-orbit-muted/50 outline-none transition-all focus:border-orbit-teal/50 focus:shadow-[0_0_20px_-5px_rgba(60,171,156,0.2)]"
          />
          <button
            type="submit"
            className="rounded-lg border border-orbit-teal/30 bg-orbit-teal/10 px-6 py-2.5 text-sm font-medium text-orbit-teal transition-all hover:bg-orbit-teal/20 hover:border-orbit-teal/50"
          >
            Search
          </button>
        </div>
      </form>

      <div className="mb-6 flex flex-wrap items-center gap-3">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-orbit-muted">Source:</span>
          <div className="flex gap-1.5">
            {["", "jetstream", "yellowstone", "both"].map((s) => (
              <button
                key={s}
                onClick={() => setSourceFilter(s)}
                className={`rounded-md px-3 py-1.5 text-xs font-medium transition-all ${
                  sourceFilter === s
                    ? "bg-orbit-teal/15 text-orbit-teal border border-orbit-teal/30"
                    : "bg-orbit-surface text-orbit-muted border border-orbit-border hover:text-orbit-text"
                }`}
              >
                {s === "" ? "All" : <SourceBadge source={s} />}
              </button>
            ))}
          </div>
        </div>

        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-orbit-muted">Status:</span>
          <select
            value={successFilter}
            onChange={(e) => setSuccessFilter(e.target.value)}
            className="rounded-md border border-orbit-border bg-orbit-surface px-3 py-1.5 text-xs text-orbit-text outline-none focus:border-orbit-teal/50"
          >
            <option value="">All</option>
            <option value="true">Success</option>
            <option value="false">Failed</option>
          </select>
        </div>

        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-orbit-muted">Slot Range:</span>
          <input
            type="number"
            value={slotMin}
            onChange={(e) => setSlotMin(e.target.value)}
            placeholder="Min"
            className="w-28 rounded-md border border-orbit-border bg-orbit-surface px-3 py-1.5 text-xs text-orbit-text outline-none focus:border-orbit-teal/50"
          />
          <span className="text-orbit-muted">-</span>
          <input
            type="number"
            value={slotMax}
            onChange={(e) => setSlotMax(e.target.value)}
            placeholder="Max"
            className="w-28 rounded-md border border-orbit-border bg-orbit-surface px-3 py-1.5 text-xs text-orbit-text outline-none focus:border-orbit-teal/50"
          />
        </div>
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
              ) : displayedTransactions.length === 0 ? (
                <tr>
                  <td
                    colSpan={7}
                    className="px-4 py-16 text-center text-sm text-orbit-muted"
                  >
                    No transactions found
                  </td>
                </tr>
              ) : (
                displayedTransactions.map((tx) => (
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
        onLoadMore={() => loadTransactions(true)}
      />
    </div>
  );
}
