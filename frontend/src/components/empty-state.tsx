import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { Icon } from "@phosphor-icons/react";
import type { ReactNode } from "react";

interface EmptyStateProps {
  icon: Icon;
  title: string;
  description: string;
  action?: { label: string; onClick: () => void };
  children?: ReactNode;
  className?: string;
}

/** Empty states teach the next step — they never say "nothing here". */
export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  children,
  className,
}: EmptyStateProps) {
  return (
    <div
      className={cn(
        "relative flex flex-col items-center justify-center overflow-hidden rounded-xl border border-dashed px-6 py-14 text-center",
        className,
      )}
      style={{
        backgroundImage:
          "radial-gradient(420px 180px at 50% 0%, color-mix(in oklab, var(--primary) 8%, transparent), transparent)",
      }}
    >
      <div className="border-primary/25 bg-primary/10 text-primary mb-3 rounded-full border p-3">
        <Icon className="size-5" />
      </div>
      <h3 className="text-sm font-semibold">{title}</h3>
      <p className="text-muted-foreground mt-1 max-w-sm text-sm">{description}</p>
      {action && (
        <Button className="mt-4" onClick={action.onClick}>
          {action.label}
        </Button>
      )}
      {children}
    </div>
  );
}
