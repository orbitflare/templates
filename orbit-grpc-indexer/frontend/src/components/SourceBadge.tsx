interface SourceBadgeProps {
  source: string;
}

const sourceStyles: Record<string, string> = {
  jetstream:
    "bg-orbit-teal/15 text-orbit-teal border-orbit-teal/30",
  yellowstone:
    "bg-purple-500/15 text-purple-400 border-purple-500/30",
  both:
    "bg-amber-500/15 text-amber-400 border-amber-500/30",
};

export default function SourceBadge({ source }: SourceBadgeProps) {
  const style = sourceStyles[source] || sourceStyles.jetstream;
  return (
    <span
      className={`inline-flex items-center rounded-md border px-2 py-0.5 text-xs font-medium ${style}`}
    >
      {source}
    </span>
  );
}
