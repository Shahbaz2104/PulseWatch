import { useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { toast } from "sonner";
import {
  Activity,
  ArrowLeft,
  CheckCircle2,
  Clock,
  Gauge,
  Loader2,
  Pause,
  Pencil,
  Play,
  Timer,
  TrendingUp,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { EmptyState } from "@/components/empty-state";
import { MetricCard } from "@/components/metric-card";
import { SeriesChart } from "@/components/series-chart";
import { StatusPill } from "@/components/status-pill";
import { UptimeStrip } from "@/components/uptime-strip";
import { MonitorFormDialog } from "@/features/monitors/monitor-form";
import { ApiError } from "@/lib/api";
import {
  useCheckNow,
  useCheckSummary,
  useChecks,
  useMonitor,
  usePauseMonitor,
  useResponseTimes,
  useResumeMonitor,
} from "@/hooks/use-api";
import {
  formatClock,
  formatHttpStatus,
  formatMs,
  formatPercent,
  formatRelative,
} from "@/lib/format";
import type { WindowQuery } from "@/types/api";

type SummaryWindow = Extract<WindowQuery["window"], "24h" | "7d" | "30d">;

function ConfigPanel({ monitor }: { monitor: NonNullable<ReturnType<typeof useMonitor>["data"]> }) {
  const rows: Array<[string, string]> = [
    ["Interval", `${monitor.check_interval_seconds}s`],
    ["Timeout", `${monitor.timeout_ms} ms`],
    ["Attempts", String(monitor.retry_max_attempts)],
    ["Expected status", `${monitor.expected_status_min}–${monitor.expected_status_max}`],
    ["Down after", `${monitor.consecutive_failures_threshold} failed`],
    ["Up after", `${monitor.consecutive_successes_threshold} succeeded`],
    ["Created", formatClock(monitor.created_at)],
  ];
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm font-medium">Configuration</CardTitle>
      </CardHeader>
      <CardContent>
        <dl className="grid grid-cols-1 gap-x-6 gap-y-2 text-sm sm:grid-cols-2">
          {rows.map(([label, value]) => (
            <div key={label} className="flex items-center justify-between gap-2 border-b py-1.5 last:border-0">
              <dt className="text-muted-foreground">{label}</dt>
              <dd className="font-mono">{value}</dd>
            </div>
          ))}
        </dl>
      </CardContent>
    </Card>
  );
}

export function MonitorDetailPage() {
  const { monitorId = "" } = useParams();
  const [summaryWindow] = useState<SummaryWindow>("24h");
  const [resultFilter, setResultFilter] = useState<"all" | "success" | "failure">("all");
  const [checkPage, setCheckPage] = useState(1);
  const [editOpen, setEditOpen] = useState(false);

  const monitorQuery = useMonitor(monitorId);
  const monitor = monitorQuery.data;

  const summary24 = useCheckSummary(monitorId, { window: "24h" });
  const summary7 = useCheckSummary(monitorId, { window: "7d" });
  const summary30 = useCheckSummary(monitorId, { window: "30d" });
  const summaryActive = useCheckSummary(monitorId, { window: summaryWindow });
  const series = useResponseTimes({ window: "24h" }, monitorId);
  const checks = useChecks(monitorId, {
    window: "30d",
    result: resultFilter === "all" ? undefined : resultFilter,
    page: checkPage,
    page_size: 25,
  });

  const checkNow = useCheckNow();
  const pause = usePauseMonitor();
  const resume = useResumeMonitor();

  const reportError = (error: unknown) =>
    toast.error(error instanceof ApiError ? error.message : "Action failed");

  const history = useMemo(() => checks.data?.data ?? [], [checks.data]);
  const activeBucket = useMemo(() => {
    const buckets = series.data?.series ?? [];
    return buckets.length > 0 ? buckets[buckets.length - 1] : undefined;
  }, [series.data]);

  if (monitorQuery.isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-10 w-64" />
        <div className="grid gap-4 sm:grid-cols-3">
          {Array.from({ length: 3 }).map((_, index) => (
            <Skeleton key={index} className="h-28" />
          ))}
        </div>
        <Skeleton className="h-72" />
      </div>
    );
  }

  if (monitorQuery.isError || !monitor) {
    const notFound = monitorQuery.error instanceof ApiError && monitorQuery.error.status === 404;
    return (
      <EmptyState
        icon={Activity}
        title={notFound ? "Monitor not found" : "Couldn't load this monitor"}
        description={
          notFound
            ? "It may have been deleted. Head back to the list to see what remains."
            : "The API didn't respond. Is the backend running?"
        }
        action={{ label: "Back to monitors", onClick: () => window.location.assign("/monitors") }}
      />
    );
  }

  const paused = monitor.current_status === "paused";
  const actionPending = checkNow.isPending || pause.isPending || resume.isPending;

  return (
    <>
      <div className="mb-4">
        <Link
          to="/monitors"
          className="inline-flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          <ArrowLeft className="size-3.5" /> Monitors
        </Link>
      </div>

      <div className="mb-6 flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-3">
            <h1 className="text-xl font-semibold tracking-tight">{monitor.name}</h1>
            <StatusPill
              status={monitor.current_status}
              degraded={monitor.degraded}
              live={actionPending}
            />
          </div>
          <p className="mt-1 font-mono text-sm break-all text-muted-foreground">{monitor.url}</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => setEditOpen(true)}>
            <Pencil /> Edit
          </Button>
          {paused ? (
            <Button
              size="sm"
              onClick={() =>
                resume.mutate(monitor.id, {
                  onSuccess: () => toast.success("Monitor resumed"),
                  onError: reportError,
                })
              }
              disabled={actionPending}
            >
              {resume.isPending ? <Loader2 className="size-4 animate-spin" /> : <Play />}
              Resume
            </Button>
          ) : (
            <Button
              size="sm"
              variant="outline"
              onClick={() =>
                pause.mutate(monitor.id, {
                  onSuccess: () => toast.success("Monitor paused"),
                  onError: reportError,
                })
              }
              disabled={actionPending}
            >
              {pause.isPending ? <Loader2 className="size-4 animate-spin" /> : <Pause />}
              Pause
            </Button>
          )}
          <Button
            size="sm"
            onClick={() =>
              checkNow.mutate(monitor.id, {
                onSuccess: (result) =>
                  toast.success(result.check.success ? "Check succeeded" : "Check failed", {
                    description: result.check.success
                      ? `HTTP ${result.check.http_status_code} · ${result.check.response_time_ms} ms`
                      : (result.check.error_message ?? undefined),
                  }),
                onError: reportError,
              })
            }
            disabled={paused || actionPending}
          >
            {checkNow.isPending ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <CheckCircle2 />
            )}
            Check now
          </Button>
        </div>
      </div>

      {/* Uptime across windows */}
      <div className="mb-4 grid gap-4 sm:grid-cols-3">
        {(
          [
            ["24h", summary24],
            ["7d", summary7],
            ["30d", summary30],
          ] as const
        ).map(([window, query], index) => (
          <MetricCard
            key={window}
            label={`Uptime ${window}`}
            value={query.data?.uptime_percent}
            format="percent"
            icon={TrendingUp}
            loading={query.isLoading}
            delay={index * 0.04}
            hint={
              query.data && query.data.total_checks === 0
                ? "No checks yet"
                : query.data
                  ? `${query.data.successful_checks}/${query.data.total_checks} checks`
                  : undefined
            }
          />
        ))}
      </div>

      <div className="mb-4 grid gap-4 sm:grid-cols-3">
        <MetricCard
          label="Latest response"
          value={summaryActive.data?.latest_response_time_ms}
          format="ms"
          icon={Gauge}
          loading={summaryActive.isLoading}
          delay={0.12}
          hint={monitor.last_checked_at ? formatRelative(monitor.last_checked_at) : "Never checked"}
        />
        <MetricCard
          label="Median"
          value={summaryActive.data?.median_response_time_ms}
          format="ms"
          icon={Timer}
          loading={summaryActive.isLoading}
          delay={0.16}
          hint="Successful checks only"
        />
        <MetricCard
          label="p95"
          value={summaryActive.data?.p95_response_time_ms}
          format="ms"
          icon={Clock}
          loading={summaryActive.isLoading}
          delay={0.2}
          hint="Successful checks only"
        />
      </div>

      <div className="mb-4 grid gap-4 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Response time — 24h</CardTitle>
            {series.data && (
              <span className="font-mono text-xs text-muted-foreground">
                bucket: {series.data.bucket}
              </span>
            )}
          </CardHeader>
          <CardContent>
            {series.isLoading ? (
              <Skeleton className="h-44" />
            ) : (
              <SeriesChart buckets={series.data?.series ?? []} height={190} />
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Uptime strip — 24h</CardTitle>
          </CardHeader>
          <CardContent>
            {series.isLoading ? (
              <Skeleton className="h-10" />
            ) : (
              <UptimeStrip buckets={series.data?.series ?? []} />
            )}
            {activeBucket && (
              <p className="mt-2 font-mono text-xs text-muted-foreground">
                last bucket: {activeBucket.successes}/{activeBucket.checks} checks ·{" "}
                {formatPercent(activeBucket.uptime_percent)}
              </p>
            )}
          </CardContent>
        </Card>
      </div>

      <Card className="mb-4">
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-sm font-medium">Check history</CardTitle>
          <div className="flex items-center gap-2">
            <Tabs
              value={resultFilter}
              onValueChange={(value) => {
                setResultFilter(value as typeof resultFilter);
                setCheckPage(1);
              }}
            >
              <TabsList className="h-8">
                <TabsTrigger value="all" className="text-xs">
                  All
                </TabsTrigger>
                <TabsTrigger value="success" className="text-xs">
                  Success
                </TabsTrigger>
                <TabsTrigger value="failure" className="text-xs">
                  Failure
                </TabsTrigger>
              </TabsList>
            </Tabs>
          </div>
        </CardHeader>
        <CardContent>
          {checks.isLoading ? (
            <Skeleton className="h-48" />
          ) : history.length === 0 ? (
            <p className="py-8 text-center text-sm text-muted-foreground">
              {resultFilter === "all"
                ? "No checks recorded in the last 30 days."
                : `No ${resultFilter} checks in the last 30 days.`}
            </p>
          ) : (
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow className="bg-muted/40 hover:bg-muted/40">
                    <TableHead>Time</TableHead>
                    <TableHead>Result</TableHead>
                    <TableHead>HTTP</TableHead>
                    <TableHead>Response</TableHead>
                    <TableHead>Attempts</TableHead>
                    <TableHead>Error</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {history.map((check) => (
                    <TableRow key={check.id}>
                      <TableCell className="font-mono text-xs whitespace-nowrap">
                        {formatClock(check.checked_at)}
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant={check.success ? "secondary" : "destructive"}
                          className="font-medium"
                        >
                          {check.success ? "Success" : "Failed"}
                        </Badge>
                      </TableCell>
                      <TableCell className="font-mono text-sm">
                        {formatHttpStatus(check.http_status_code)}
                      </TableCell>
                      <TableCell className="font-mono text-sm">
                        {formatMs(check.response_time_ms)}
                      </TableCell>
                      <TableCell className="font-mono text-sm">{check.attempt_count}</TableCell>
                      <TableCell className="max-w-56 truncate text-xs text-muted-foreground">
                        {check.error_message ?? check.error_type ?? "—"}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
          {checks.data && checks.data.pagination.total_pages > 1 && (
            <div className="mt-3 flex items-center justify-between text-xs text-muted-foreground">
              <span>
                Page {checks.data.pagination.page} of {checks.data.pagination.total_pages}
              </span>
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={checkPage <= 1}
                  onClick={() => setCheckPage((value) => Math.max(1, value - 1))}
                >
                  Previous
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={checkPage >= checks.data.pagination.total_pages}
                  onClick={() => setCheckPage((value) => value + 1)}
                >
                  Next
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      <ConfigPanel monitor={monitor} />

      <MonitorFormDialog open={editOpen} onOpenChange={setEditOpen} editing={monitor} />
    </>
  );
}
