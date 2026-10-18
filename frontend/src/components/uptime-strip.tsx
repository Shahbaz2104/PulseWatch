import { format } from "date-fns";
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

/** Vertical per-bucket bars — the status-page "uptime strip" at a glance. */
export function UptimeStrip({ buckets, className }: UptimeStripProps) {
  if (buckets.length === 0) {
    return (
      <p className="py-6 text-center text-sm text-muted-foreground">No data in this range.</p>
    );
  }
  return (
    <div
      className={cn("flex h-10 items-end gap-px", className)}
      role="img"
      aria-label="Uptime per period"
    >
      {buckets.map((bucket, index) => (
        <motion.div
          key={bucket.bucket_start}
          title={bucketTitle(bucket)}
          className={cn(
            "min-w-0 flex-1 rounded-[1px] transition-opacity hover:opacity-70",
            bucketClass(bucket),
          )}
          style={{ height: "100%", transformOrigin: "bottom" }}
          initial={{ scaleY: 0, opacity: 0.4 }}
          animate={{ scaleY: 1, opacity: 1 }}
          transition={{
            duration: 0.4,
            delay: Math.min(index * 0.012, 0.45),
            ease: [0.21, 0.47, 0.32, 0.98],
          }}
        />
      ))}
    </div>
  );
}
