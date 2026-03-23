import Link from "next/link";
import SourceBadge from "./SourceBadge";
import type { Transaction } from "@/lib/api";

interface TransactionRowProps {
  tx: Transaction;
}

function truncateSig(sig: string): string {
  if (sig.length <= 16) return sig;
  return `${sig.slice(0, 8)}...${sig.slice(-8)}`;
}

function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export default function TransactionRow({ tx }: TransactionRowProps) {
  return (
    <tr className="group border-b border-orbit-border/50 transition-colors hover:bg-orbit-teal/5">
      <td className="whitespace-nowrap px-4 py-3">
        <Link
          href={`/transactions/${tx.signature}`}
          className="font-mono text-sm text-orbit-teal transition-colors hover:text-orbit-mint"
        >
          {truncateSig(tx.signature)}
        </Link>
      </td>
      <td className="whitespace-nowrap px-4 py-3 font-mono text-sm text-orbit-text">
        {tx.slot.toLocaleString()}
      </td>
      <td className="whitespace-nowrap px-4 py-3">
        <SourceBadge source={tx.source} />
      </td>
      <td className="whitespace-nowrap px-4 py-3">
        {tx.success ? (
          <span className="inline-flex items-center gap-1.5 text-sm text-orbit-success">
            <span className="h-1.5 w-1.5 rounded-full bg-orbit-success" />
            Success
          </span>
        ) : (
          <span className="inline-flex items-center gap-1.5 text-sm text-orbit-error">
            <span className="h-1.5 w-1.5 rounded-full bg-orbit-error" />
            Failed
          </span>
        )}
      </td>
      <td className="whitespace-nowrap px-4 py-3 text-sm text-orbit-muted">
        {tx.num_instructions}
      </td>
      <td className="whitespace-nowrap px-4 py-3">
        {tx.has_cpi_data ? (
          <span className="text-xs text-orbit-mint">Yes</span>
        ) : (
          <span className="text-xs text-orbit-muted">No</span>
        )}
      </td>
      <td className="whitespace-nowrap px-4 py-3 text-sm text-orbit-muted">
        {formatDate(tx.indexed_at)}
      </td>
    </tr>
  );
}
