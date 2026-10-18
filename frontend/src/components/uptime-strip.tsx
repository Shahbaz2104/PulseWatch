import { format } from "date-fns";
import { useState } from "react";
import { motion } from "motion/react";
import type { TimeBucket } from "@/types/api";
import { cn } from "@/lib/utils";

interface UptimeStripProps {
  buckets: TimeBucket[];
  className?: string;
}

function bucketTitle(bucket: TimeBucket): string {
  const start = new Date(bucket.bucket_start);
  const time = format(start, "HH:mm");
  if (bucket.checks === 0) return `${time} — no checks`;
  const uptime = bucket.uptime_percent == null ? "—" : `${bucket.uptime_percent.toFixed(1)}%`;
  return `${time} — ${uptime} uptime, ${bucket.successes}/${bucket.checks} checks`;
}

function bucketClass(bucket: TimeBucket): string {
  if (bucket.checks === 0) return "bg-muted";
  if (bucket.uptime_percent == null) return "bg-muted";
  if (bucket.uptime_percent >= 99.5) return "bg-success";
  if (bucket.uptime_percent >= 90) return "bg-warning";
  return "bg-danger";
}

/** The API only returns non-empty buckets. Left alone, a monitor that started
 * 40 minutes ago renders one full-width red bar — visually claiming a whole
 * day of outage. Pad the window (24 hourly slots) with honest "no checks"
 * slots so bar widths keep their time meaning. */
function padToWindow(buckets: TimeBucket[]): TimeBucket[] {
  if (buckets.length === 0) return buckets;
  const pairs = buckets
    .map((b) => [new Date(b.bucket_start).getTime(), b] as const)
    .sort((a, b) => a[0] - b[0]);
  const times = pairs.map(([t]) => t);
  let step = 3_600_000;
  if (times.length >= 2) {
    const gaps = times
      .slice(1)
      .map((t, i) => t - times[i])
      .filter((g) => g > 0);
    if (gaps.length > 0) step = Math.min(...gaps);
  }
  const windowMs = step <= 2 * 3_600_000 ? 24 * 3_600_000 : step * 24;
  const start = Math.floor((Date.now() - windowMs) / step) * step;
  const end = Math.ceil(Date.now() / step) * step;
  const slots = Math.round((end - start) / step);
  if (slots <= 0 || slots > 400) return buckets;

  const byTime = new Map(pairs);
  const padded: TimeBucket[] = [];
  for (let t = start; t < end; t += step) {
    padded.push(
      byTime.get(t) ?? {
        bucket_start: new Date(t).toISOString(),
        checks: 0,
        successes: 0,
        uptime_percent: null,
        avg_response_time_ms: null,
      },
    );
  }
  return padded;
}

/** Segmented per-bucket bars — the status-page "uptime strip" at a glance,
 * with a hover tooltip per segment. */
export function UptimeStrip({ buckets: rawBuckets, className }: UptimeStripProps) {
  const [hovered, setHovered] = useState<number | null>(null);
  const buckets = padToWindow(rawBuckets);

  if (buckets.length === 0) {
    return <p className="text-muted-foreground py-6 text-center text-sm">No data in this range.</p>;
  }

  const n = buckets.length;
  const hoveredBucket = hovered != null ? buckets[hovered] : null;
  const tooltipLeft =
    hovered != null ? Math.min(Math.max(((hovered + 0.5) / n) * 100, 12), 88) : 50;
  const first = buckets[0];
  const last = buckets[n - 1];

  return (
    <div className={cn("relative", className)}>
      <div
        className="group flex h-11 items-end gap-[2px]"
        role="img"
        aria-label={`Uptime per period: ${bucketTitle(first)} to ${bucketTitle(last)}`}
        onMouseLeave={() => setHovered(null)}
      >
        {buckets.map((bucket, index) => (
          <motion.div
            key={bucket.bucket_start}
            title={bucketTitle(bucket)}
            onMouseEnter={() => setHovered(index)}
            className={cn(
              "min-w-0 flex-1 rounded-[2px] transition-[opacity,filter] duration-150",
              bucketClass(bucket),
              hovered == null || hovered === index ? "opacity-100" : "opacity-45",
              hovered === index && "brightness-125",
            )}
            style={{ height: "100%", transformOrigin: "bottom" }}
            initial={{ scaleY: 0 }}
            animate={{ scaleY: 1 }}
            transition={{
              duration: 0.4,
              delay: Math.min(index * 0.012, 0.45),
              ease: [0.21, 0.47, 0.32, 0.98],
            }}
          />
        ))}
      </div>

      {hoveredBucket && (
        <div
          className="bg-popover shadow-pop pointer-events-none absolute -top-9 z-20 -translate-x-1/2 rounded-md border px-2.5 py-1.5 text-xs whitespace-nowrap"
          style={{ left: `${tooltipLeft}%` }}
          role="tooltip"
        >
          <span className="tabular font-mono">{bucketTitle(hoveredBucket)}</span>
        </div>
      )}

      <div className="text-muted-foreground tabular mt-1.5 flex justify-between font-mono text-[11px]">
        <span>{format(new Date(first.bucket_start), "HH:mm")}</span>
        <span>{format(new Date(last.bucket_start), "HH:mm")}</span>
      </div>
    </div>
  );
}
