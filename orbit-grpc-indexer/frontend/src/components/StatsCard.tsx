interface StatsCardProps {
  label: string;
  value: string | number;
  sub?: string;
  accent?: boolean;
}

export default function StatsCard({ label, value, sub, accent }: StatsCardProps) {
  return (
    <div className="group relative overflow-hidden rounded-xl border border-orbit-border bg-orbit-surface p-5 transition-all duration-300 hover:border-orbit-teal/30 hover:shadow-[0_0_30px_-5px_rgba(60,171,156,0.15)]">
      <div className="absolute inset-0 bg-gradient-to-br from-orbit-teal/5 to-transparent opacity-0 transition-opacity duration-300 group-hover:opacity-100" />
      <div className="relative">
        <p className="text-xs font-medium uppercase tracking-wider text-orbit-muted">
          {label}
        </p>
        <p
          className={`mt-2 text-2xl font-bold tracking-tight ${
            accent ? "text-orbit-teal" : "text-orbit-text"
          }`}
        >
          {value}
        </p>
        {sub && (
          <p className="mt-1 text-xs text-orbit-muted">{sub}</p>
        )}
      </div>
    </div>
  );
}
