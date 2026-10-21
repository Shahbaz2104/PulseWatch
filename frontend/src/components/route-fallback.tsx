import { CircleNotch } from "@phosphor-icons/react";

/** Suspense fallback for lazily loaded route chunks. */
export function RouteFallback({ fullScreen = false }: { fullScreen?: boolean }) {
  return (
    <div
      className={
        fullScreen
          ? "flex min-h-svh items-center justify-center gap-2"
          : "flex min-h-[45vh] items-center justify-center gap-2"
      }
      role="status"
      aria-live="polite"
    >
      <CircleNotch className="text-muted-foreground size-4 animate-spin" aria-hidden />
      <span className="text-muted-foreground text-sm">Loading…</span>
    </div>
  );
}
