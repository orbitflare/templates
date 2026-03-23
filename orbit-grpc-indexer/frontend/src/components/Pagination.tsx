"use client";

interface PaginationProps {
  hasMore: boolean;
  loading: boolean;
  onLoadMore: () => void;
}

export default function Pagination({
  hasMore,
  loading,
  onLoadMore,
}: PaginationProps) {
  if (!hasMore) {
    return (
      <div className="py-8 text-center text-sm text-orbit-muted">
        No more results
      </div>
    );
  }

  return (
    <div className="flex justify-center py-8">
      <button
        onClick={onLoadMore}
        disabled={loading}
        className="group relative overflow-hidden rounded-lg border border-orbit-teal/30 bg-orbit-teal/10 px-6 py-2.5 text-sm font-medium text-orbit-teal transition-all duration-200 hover:bg-orbit-teal/20 hover:border-orbit-teal/50 hover:shadow-[0_0_20px_-5px_rgba(60,171,156,0.3)] disabled:cursor-not-allowed disabled:opacity-50"
      >
        <span className="relative">
          {loading ? "Loading..." : "Load More"}
        </span>
      </button>
    </div>
  );
}
