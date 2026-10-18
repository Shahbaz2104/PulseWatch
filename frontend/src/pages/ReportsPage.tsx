import { useState } from "react";
import { Link } from "react-router-dom";
import { BarChart3, Download } from "lucide-react";
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
  rows: Array<{ name: string; url: string; status: string; checks: number; successes: number; uptime: string }>,
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
  const windowLabel = windowQuery.window ?? "custom";

  const uptime = useUptimeReport(windowQuery);
  const times = useResponseTimes(windowQuery);
  const incidentReport = useIncidentReport(windowQuery);

  const isLoading = uptime.isLoading || times.isLoading || incidentReport.isLoading;
  const hasMonitors = (uptime.data?.monitors.length ?? 0) > 0;

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
                    {uptime.data.monitors.map((row) => (
                      <TableRow key={row.monitor_id}>
                        <TableCell>
                          <Link
                            to={`/monitors/${row.monitor_id}`}
                            className="font-medium hover:text-primary hover:underline"
                          >
                            {row.name}
                          </Link>
                          <p className="truncate font-mono text-xs text-muted-foreground">
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
            </CardContent>
          </Card>

          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium">
                  Response time — {windowLabel}
                </CardTitle>
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
                      {times.data?.monitors.map((row) => (
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
                              <span className="text-muted-foreground" title="Needs at least 2 samples">
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
                <p className="mt-3 text-xs text-muted-foreground">
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
