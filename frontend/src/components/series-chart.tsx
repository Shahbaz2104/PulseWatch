import {
  Area,
  AreaChart,
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
      <p className="py-6 text-center text-sm text-muted-foreground">
        No response-time data yet.
      </p>
    );
  }
  const points = toPoints(buckets);
  return (
    <ResponsiveContainer width="100%" height={height}>
      <AreaChart data={points} margin={{ top: 8, right: 8, left: -12, bottom: 0 }}>
        <defs>
          <linearGradient id="rtFill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--color-chart-1)" stopOpacity={0.25} />
            <stop offset="100%" stopColor="var(--color-chart-1)" stopOpacity={0} />
          </linearGradient>
        </defs>
        <XAxis
          dataKey="time"
          type="number"
          domain={["dataMin", "dataMax"]}
          tickFormatter={(value: number) => format(new Date(value), "HH:mm")}
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
          }}
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
          dot={false}
          isAnimationActive={false}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}
