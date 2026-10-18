import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import type { WindowQuery } from "@/types/api";
import { cn } from "@/lib/utils";

const PRESETS = ["24h", "7d", "30d"] as const;
type Preset = (typeof PRESETS)[number];

interface WindowPickerProps {
  value: WindowQuery;
  onChange: (value: WindowQuery) => void;
  /** Hide the custom daterange where it makes no sense (e.g. 24h sparklines). */
  allowCustom?: boolean;
  className?: string;
}

function toIsoLocal(date: string): string | undefined {
  if (!date) return undefined;
  const parsed = new Date(`${date}T00:00:00`);
  if (Number.isNaN(parsed.getTime())) return undefined;
  return parsed.toISOString();
}

export function WindowPicker({
  value,
  onChange,
  allowCustom = true,
  className,
}: WindowPickerProps) {
  const current: Preset | "custom" =
    value.window && (PRESETS as readonly string[]).includes(value.window)
      ? (value.window as Preset)
      : value.window === "custom"
        ? "custom"
        : "24h";

  return (
    <div className={cn("flex flex-wrap items-center gap-2", className)}>
      <Tabs
        value={current}
        onValueChange={(next) => {
          if (next === "custom") {
            onChange({
              window: "custom",
              from: value.from ?? new Date(Date.now() - 7 * 86_400_000).toISOString(),
              to: value.to,
            });
          } else {
            onChange({ window: next as WindowQuery["window"] });
          }
        }}
      >
        <TabsList>
          {PRESETS.map((preset) => (
            <TabsTrigger key={preset} value={preset}>
              {preset}
            </TabsTrigger>
          ))}
          {allowCustom && <TabsTrigger value="custom">Custom</TabsTrigger>}
        </TabsList>
      </Tabs>
      {current === "custom" && (
        <div className="flex items-center gap-2">
          <Input
            type="date"
            aria-label="Window start"
            className="h-8 w-36 font-mono text-xs"
            value={value.from ? value.from.slice(0, 10) : ""}
            onChange={(event) =>
              onChange({ ...value, window: "custom", from: toIsoLocal(event.target.value) })
            }
          />
          <span className="text-muted-foreground text-xs">to</span>
          <Input
            type="date"
            aria-label="Window end"
            className="h-8 w-36 font-mono text-xs"
            value={value.to ? value.to.slice(0, 10) : ""}
            onChange={(event) =>
              onChange({ ...value, window: "custom", to: toIsoLocal(event.target.value) })
            }
          />
          <Button
            variant="ghost"
            size="sm"
            className="h-8 text-xs"
            onClick={() => onChange({ window: "24h" })}
          >
            Reset
          </Button>
        </div>
      )}
    </div>
  );
}
