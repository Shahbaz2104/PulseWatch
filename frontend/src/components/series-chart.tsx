import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { format } from "date-fns";
import type { TimeBucket } from "@/types/api";

interface SeriesChartProps {
  buckets: TimeBucket[];
  height?: number;
}

interface Point {
  time: number;
  avg: number | null;
  uptime: number | null;
}

function toPoints(buckets: TimeBucket[]): Point[] {
  return buckets.map((bucket) => ({
    time: new Date(bucket.bucket_start).getTime(),
    avg: bucket.avg_response_time_ms,
    uptime: bucket.uptime_percent,
  }));
}

/** Response-time area chart over aggregated buckets. Empty buckets render as
 * gaps (null), never as zero. */
export function SeriesChart({ buckets, height = 180 }: SeriesChartProps) {
  if (buckets.length === 0) {
    return (
      <p className="text-muted-foreground py-6 text-center text-sm">No response-time data yet.</p>
    );
  }
  const points = toPoints(buckets);
  // A lone bucket renders no path — widen the time domain so it isn't at the
  // canvas edge, and draw the point as a dot instead of an invisible line.
  const first = points[0];
  const singlePoint = points.length === 1 && first !== undefined;
  const timeDomain = singlePoint
    ? ([first.time - 60_000, first.time + 60_000] as [number, number])
    : (["dataMin", "dataMax"] as [string, string]);
  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={points} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
        <defs>
          <linearGradient id="rtFill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--color-chart-1)" stopOpacity={0.25} />
            <stop offset="100%" stopColor="var(--color-chart-1)" stopOpacity={0} />
          </linearGradient>
        </defs>
        <CartesianGrid vertical={false} stroke="var(--color-border)" strokeDasharray="2 4" />
        <XAxis
          dataKey="time"
          type="number"
          domain={timeDomain}
          ticks={singlePoint && first ? [first.time] : undefined}
          tickFormatter={(value: number) =>
            singlePoint ? format(new Date(value), "MMM d, HH:mm") : format(new Date(value), "HH:mm")
          }
          tick={{ fontSize: 11, fill: "var(--color-muted-foreground)" }}
          axisLine={false}
          tickLine={false}
          minTickGap={40}
        />
        <YAxis
          tickFormatter={(value: number) => `${value}`}
          tick={{ fontSize: 11, fill: "var(--color-muted-foreground)" }}
          axisLine={false}
          tickLine={false}
          width={48}
          unit="ms"
        />
        <Tooltip
          contentStyle={{
            background: "var(--color-popover)",
            border: "1px solid var(--color-border)",
            borderRadius: "var(--radius)",
            fontSize: 12,
            boxShadow: "var(--shadow-pop)",
          }}
          cursor={{ stroke: "var(--color-border)", strokeWidth: 1 }}
          labelFormatter={(value) => format(new Date(Number(value)), "MMM d, HH:mm")}
          formatter={(value, name) => [
            value == null ? "—" : `${Number(value).toFixed(0)} ms`,
            name === "avg" ? "Avg response" : String(name),
          ]}
        />
        <Area
          type="monotone"
          dataKey="avg"
          stroke="var(--color-chart-1)"
          strokeWidth={1.75}
          fill="url(#rtFill)"
          connectNulls={false}
          dot={singlePoint ? { r: 3.5, fill: "var(--color-chart-1)", strokeWidth: 0 } : false}
          isAnimationActive={false}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}
