import { useState } from "react";
import { Link } from "react-router-dom";
import { ChartBar as BarChart3, Download } from "@phosphor-icons/react";
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
import { EmptyState } from "@/components/empty-state";
import { MetricCard } from "@/components/metric-card";
import { PageHeader } from "@/components/page-header";
import { SeriesChart } from "@/components/series-chart";
import { StatusPill } from "@/components/status-pill";
import { WindowPicker } from "@/components/window-picker";
import { useIncidentReport, useResponseTimes, useUptimeReport } from "@/hooks/use-api";
import { formatMs, formatPercent, formatSeconds } from "@/lib/format";
import type { WindowQuery } from "@/types/api";

/** Download the current uptime table as CSV (client-side, no backend work). */
function exportUptimeCsv(
  rows: Array<{
    name: string;
    url: string;
    status: string;
    checks: number;
    successes: number;
    uptime: string;
  }>,
  windowLabel: string,
) {
  const header = ["monitor", "url", "status", "checks", "successes", "uptime_percent"];
  const lines = rows.map((row) =>
    [row.name, row.url, row.status, row.checks, row.successes, row.uptime]
      .map((cell) => `"${String(cell).replaceAll('"', '""')}"`)
      .join(","),
  );
  const csv = [header.join(","), ...lines].join("\n");
  const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `pulsewatch-uptime-${windowLabel}-${new Date().toISOString().slice(0, 10)}.csv`;
  anchor.click();
  URL.revokeObjectURL(url);
}

export function ReportsPage() {
  const [windowQuery, setWindowQuery] = useState<WindowQuery>({ window: "7d" });
  const [uptimePage, setUptimePage] = useState(1);
  const [timesPage, setTimesPage] = useState(1);
  const windowLabel = windowQuery.window ?? "custom";

  const uptime = useUptimeReport(windowQuery);
  const times = useResponseTimes(windowQuery);
  const incidentReport = useIncidentReport(windowQuery);

  const isLoading = uptime.isLoading || times.isLoading || incidentReport.isLoading;
  const hasMonitors = (uptime.data?.monitors.length ?? 0) > 0;

  // Both report tables return every monitor; at 100+ monitors a single table
  // becomes a scroll wall, so each paginates client-side (20 rows/page,
  // matching Monitors). Pages clamp when the window changes.
  const uptimeRows = uptime.data?.monitors ?? [];
  const uptimeTotal = Math.max(1, Math.ceil(uptimeRows.length / 20));
  const uptimeIndex = Math.min(uptimePage, uptimeTotal);
  const uptimeSlice = uptimeRows.slice((uptimeIndex - 1) * 20, uptimeIndex * 20);

  const timesRows = times.data?.monitors ?? [];
  const timesTotal = Math.max(1, Math.ceil(timesRows.length / 20));
  const timesIndex = Math.min(timesPage, timesTotal);
  const timesSlice = timesRows.slice((timesIndex - 1) * 20, timesIndex * 20);

  const csvRows =
    uptime.data?.monitors.map((row) => ({
      name: row.name,
      url: row.url,
      status: row.current_status,
      checks: row.checks,
      successes: row.successes,
      uptime: row.uptime_percent == null ? "" : row.uptime_percent.toFixed(3),
    })) ?? [];

  return (
    <>
      <PageHeader
        title="Reports"
        description="Aggregated uptime, latency percentiles, and downtime."
        actions={
          hasMonitors ? (
            <Button
              variant="outline"
              size="sm"
              onClick={() => exportUptimeCsv(csvRows, windowLabel)}
            >
              <Download /> Export CSV
            </Button>
          ) : undefined
        }
      />

      <WindowPicker value={windowQuery} onChange={setWindowQuery} className="mb-6" />

      {isLoading && (
        <div className="space-y-4">
          <div className="grid gap-4 sm:grid-cols-3">
            {Array.from({ length: 3 }).map((_, index) => (
              <Skeleton key={index} className="h-28" />
            ))}
          </div>
          <Skeleton className="h-64" />
        </div>
      )}

      {uptime.data && !hasMonitors && (
        <EmptyState
          icon={BarChart3}
          title="Nothing to report yet"
          description="Reports populate once monitors have completed checks inside the selected window."
        />
      )}

      {uptime.data && hasMonitors && (
        <div className="space-y-6">
          <div className="grid gap-4 sm:grid-cols-3">
            <MetricCard
              label="Overall uptime"
              value={uptime.data.overall_uptime_percent}
              format="percent"
              delay={0}
              hint={`${uptime.data.total_successes}/${uptime.data.total_checks} checks`}
            />
            <MetricCard
              label="Outages"
              value={incidentReport.data?.stats.outage_count ?? 0}
              format="integer"
              delay={0.04}
              hint="Incidents started in window"
            />
            <MetricCard
              label="Downtime"
              value={
                incidentReport.data
                  ? formatSeconds(incidentReport.data.stats.downtime_seconds)
                  : "—"
              }
              delay={0.08}
              hint="Open incidents measured to now"
            />
          </div>

          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium">Uptime by monitor</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="overflow-x-auto">
                <Table>
                  <TableHeader>
                    <TableRow className="bg-muted/40 hover:bg-muted/40">
                      <TableHead>Monitor</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead>Checks</TableHead>
                      <TableHead>Successes</TableHead>
                      <TableHead>Uptime</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {uptimeSlice.map((row) => (
                      <TableRow key={row.monitor_id}>
                        <TableCell>
                          <Link
                            to={`/monitors/${row.monitor_id}`}
                            className="hover:text-primary font-medium hover:underline"
                          >
                            {row.name}
                          </Link>
                          <p className="text-muted-foreground truncate font-mono text-xs">
                            {row.url}
                          </p>
                        </TableCell>
                        <TableCell>
                          <StatusPill status={row.current_status} />
                        </TableCell>
                        <TableCell className="font-mono text-sm">{row.checks}</TableCell>
                        <TableCell className="font-mono text-sm">{row.successes}</TableCell>
                        <TableCell className="font-mono text-sm">
                          {formatPercent(row.uptime_percent)}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
              {uptimeTotal > 1 && (
                <div className="text-muted-foreground mt-4 flex items-center justify-between text-sm">
                  <span>
                    Page {uptimeIndex} of {uptimeTotal} · {uptimeRows.length} monitor
                    {uptimeRows.length === 1 ? "" : "s"}
                  </span>
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={uptimeIndex <= 1}
                      onClick={() => setUptimePage(Math.max(1, uptimeIndex - 1))}
                    >
                      Previous
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={uptimeIndex >= uptimeTotal}
                      onClick={() => setUptimePage(uptimeIndex + 1)}
                    >
                      Next
                    </Button>
                  </div>
                </div>
              )}
            </CardContent>
          </Card>

          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium">Response time — {windowLabel}</CardTitle>
              </CardHeader>
              <CardContent>
                <SeriesChart buckets={times.data?.series ?? []} height={200} />
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium">Latency percentiles</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="overflow-x-auto">
                  <Table>
                    <TableHeader>
                      <TableRow className="bg-muted/40 hover:bg-muted/40">
                        <TableHead>Monitor</TableHead>
                        <TableHead>Samples</TableHead>
                        <TableHead>Avg</TableHead>
                        <TableHead>Median</TableHead>
                        <TableHead>p95</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {timesSlice.map((row) => (
                        <TableRow key={row.monitor_id}>
                          <TableCell className="font-medium">{row.name}</TableCell>
                          <TableCell className="font-mono text-sm">{row.samples}</TableCell>
                          <TableCell className="font-mono text-sm">
                            {row.samples === 0 ? (
                              <span className="text-muted-foreground">—</span>
                            ) : (
                              formatMs(row.avg_response_time_ms)
                            )}
                          </TableCell>
                          <TableCell className="font-mono text-sm">
                            {row.samples < 2 ? (
                              <span
                                className="text-muted-foreground"
                                title="Needs at least 2 samples"
                              >
                                —
                              </span>
                            ) : (
                              formatMs(row.median_response_time_ms)
                            )}
                          </TableCell>
                          <TableCell className="font-mono text-sm">
                            {row.samples < 2 ? (
                              <span className="text-muted-foreground">—</span>
                            ) : (
                              formatMs(row.p95_response_time_ms)
                            )}
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </div>
                {timesTotal > 1 && (
                  <div className="text-muted-foreground mt-4 flex items-center justify-between text-sm">
                    <span>
                      Page {timesIndex} of {timesTotal} · {timesRows.length} monitor
                      {timesRows.length === 1 ? "" : "s"}
                    </span>
                    <div className="flex gap-2">
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={timesIndex <= 1}
                        onClick={() => setTimesPage(Math.max(1, timesIndex - 1))}
                      >
                        Previous
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={timesIndex >= timesTotal}
                        onClick={() => setTimesPage(timesIndex + 1)}
                      >
                        Next
                      </Button>
                    </div>
                  </div>
                )}
                <p className="text-muted-foreground mt-3 text-xs">
                  Median and p95 use exact nearest-rank over successful checks; fewer than two
                  samples shows “—” instead of a fabricated value.
                </p>
              </CardContent>
            </Card>
          </div>
        </div>
      )}
    </>
  );
}
