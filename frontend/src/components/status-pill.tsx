import { cn } from "@/lib/utils";
import type { MonitorStatus } from "@/types/api";

const STATUS_LABEL: Record<MonitorStatus, string> = {
  up: "Up",
  down: "Down",
  unknown: "Unknown",
  paused: "Paused",
};

const STATUS_CLASS: Record<MonitorStatus | "degraded", string> = {
  up: "text-status-up bg-success/10 border-success/25",
  down: "text-status-down bg-danger/10 border-danger/25",
  degraded: "text-status-degraded bg-warning/10 border-warning/25",
  unknown: "text-status-unknown bg-unknown/10 border-unknown/25",
  paused: "text-status-paused bg-muted border-border",
};

function dotClass(status: MonitorStatus): string {
  switch (status) {
    case "up":
      return "bg-success";
    case "down":
      return "bg-danger";
    case "paused":
      return "bg-unknown";
    default:
      return "bg-unknown";
  }
}

function dotTone(status: MonitorStatus | "degraded"): string {
  switch (status) {
    case "up":
    case "degraded":
      return status === "degraded" ? "text-warning" : "text-success";
    case "down":
      return "text-danger";
    default:
      return "text-unknown";
  }
}

interface StatusPillProps {
  status: MonitorStatus;
  /** Nominally up, but recovery threshold not yet met — shown amber. */
  degraded?: boolean;
  /** Animate the dot (check currently in flight). */
  live?: boolean;
  /** Override the status-derived text (e.g. incidents: Open / Resolved). */
  label?: string;
  className?: string;
}

export function StatusPill({ status, degraded, live, label, className }: StatusPillProps) {
  const effective = degraded && status === "up" ? "degraded" : status;
  const text = label ?? (effective === "degraded" ? "Degraded" : STATUS_LABEL[status]);
  const key = effective as keyof typeof STATUS_CLASS;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-xs font-medium",
        STATUS_CLASS[key],
        className,
      )}
      data-status={effective}
    >
      <span className="relative flex size-1.5">
        <span
          className={cn(
            "size-1.5 rounded-full",
            effective === "degraded" ? "bg-warning" : dotClass(status),
            live && "animate-pulse-dot",
          )}
        />
        {live && (
          <span
            aria-hidden
            className={cn(
              "animate-ring-pulse absolute inset-0 rounded-full",
              effective === "degraded" ? "text-warning" : dotTone(status),
            )}
          />
        )}
      </span>
      {text}
    </span>
  );
}
