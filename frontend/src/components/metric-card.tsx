import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import NumberFlow from "@number-flow/react";
import { motion } from "motion/react";
import type { LucideIcon } from "lucide-react";

export type MetricFormat = "integer" | "ms" | "percent" | "raw";

interface MetricCardProps {
  label: string;
  /** Numeric value renders as an animated NumberFlow; `null` shows an em dash. */
  value?: number | string | null;
  /** How to format a numeric `value`. Ignored for pre-formatted strings. */
  format?: MetricFormat;
  icon?: LucideIcon;
  hint?: string;
  /** Tone shifts the value color: status colors only, never decoration. */
  tone?: "default" | "up" | "down" | "warning";
  loading?: boolean;
  className?: string;
  /** Entrance delay (seconds) for staggered grids. */
  delay?: number;
}

const TONE_CLASS = {
  default: "",
  up: "text-status-up",
  down: "text-status-down",
  warning: "text-status-degraded",
} as const;

function AnimatedValue({ value, format }: { value: number; format: MetricFormat }) {
  if (format === "ms") {
    return (
      <NumberFlow
        value={value}
        format={{ maximumFractionDigits: 0 }}
        suffix=" ms"
        aria-label={`${Math.round(value)} milliseconds`}
      />
    );
  }
  if (format === "percent") {
    return (
      <NumberFlow
        value={value}
        format={{ maximumFractionDigits: 2 }}
        suffix="%"
        aria-label={`${value.toFixed(2)} percent`}
      />
    );
  }
  if (format === "integer") {
    return (
      <NumberFlow
        value={value}
        format={{ maximumFractionDigits: 0 }}
        aria-label={String(Math.round(value))}
      />
    );
  }
  return <NumberFlow value={value} />;
}

export function MetricCard({
  label,
  value,
  format = "raw",
  icon: Icon,
  hint,
  tone = "default",
  loading,
  className,
  delay = 0,
}: MetricCardProps) {
  const numeric = typeof value === "number" && Number.isFinite(value);
  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.28, delay, ease: [0.21, 0.47, 0.32, 0.98] }}
      className="h-full"
    >
      <Card className={cn("card-hover gap-0 py-4 h-full", className)}>
        <CardContent className="flex items-start justify-between gap-3 px-4">
          <div className="min-w-0">
            <p className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
              {label}
            </p>
            {loading ? (
              <Skeleton className="mt-2 h-8 w-20" />
            ) : (
              <p
                className={cn(
                  "mt-1 font-mono text-2xl font-semibold tabular",
                  TONE_CLASS[tone],
                )}
              >
                {numeric ? (
                  <AnimatedValue value={value as number} format={format} />
                ) : (
                  (value ?? "—")
                )}
              </p>
            )}
            {hint && !loading && (
              <p className="mt-1 text-xs text-muted-foreground">{hint}</p>
            )}
          </div>
          {Icon && (
            <div className="rounded-md border bg-muted/50 p-2 text-muted-foreground transition-colors group-hover:text-primary">
              <Icon className="size-4" />
            </div>
          )}
        </CardContent>
      </Card>
    </motion.div>
  );
}
