import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Heartbeat,
  Warning as AlertTriangle,
  ArrowRight,
  CheckCircle as CheckCircle2,
  Question as CircleHelp,
  Gauge,
  PauseCircle,
  Plus,
  Timer,
  TrendUp as TrendingUp,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/empty-state";
import { MetricCard } from "@/components/metric-card";
import { PageHeader } from "@/components/page-header";
import { StatusPill } from "@/components/status-pill";
import { UptimeStrip } from "@/components/uptime-strip";
import { SeriesChart } from "@/components/series-chart";
import { WindowPicker } from "@/components/window-picker";
import { useOverview } from "@/hooks/use-api";
import { formatMs, formatRelative, formatSeconds } from "@/lib/format";
import type { WindowQuery } from "@/types/api";

export function OverviewPage() {
  const navigate = useNavigate();
  const [windowQuery, setWindowQuery] = useState<WindowQuery>({ window: "24h" });
  const { data, isLoading, isError, refetch, isFetching } = useOverview(windowQuery);

  const hasMonitors = (data?.monitors.total ?? 0) > 0;
  const windowLabel =
    windowQuery.window === "custom" ? "custom window" : `last ${windowQuery.window}`;

  return (
    <>
      <PageHeader
        title="Overview"
        description="Fleet health at a glance — real check data, never fabricated."
        actions={
          <Button onClick={() => navigate("/monitors?new=1")}>
            <Plus /> Add monitor
          </Button>
        }
      />

      {hasMonitors && (
        <div className="mb-4 flex justify-end">
          <WindowPicker value={windowQuery} onChange={setWindowQuery} />
        </div>
      )}

      {isLoading && (
        <div className="space-y-4">
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {Array.from({ length: 4 }).map((_, index) => (
              <Skeleton key={index} className="h-28" />
            ))}
          </div>
          <Skeleton className="h-64" />
        </div>
      )}

      {isError && (
        <EmptyState
          icon={AlertTriangle}
          title="Couldn't load the overview"
          description="The API didn't respond. Is the backend running on port 3000?"
          action={{ label: "Retry", onClick: () => void refetch() }}
        />
      )}

      {data && !hasMonitors && (
        <EmptyState
          icon={Heartbeat}
          title="No monitors yet"
          description="Add your first website URL — PulseWatch starts checking immediately and the dashboard fills with real data."
          action={{ label: "Add your first monitor", onClick: () => navigate("/monitors?new=1") }}
        />
      )}

      {data && hasMonitors && (
        <div className="space-y-6">
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <MetricCard
              label="Total monitors"
              value={data.monitors.total}
              format="integer"
              icon={Heartbeat}
              loading={isFetching && !data}
              delay={0}
            />
            <MetricCard
              label="Operational"
              value={data.monitors.up}
              format="integer"
              icon={CheckCircle2}
              tone="up"
              hint={data.monitors.paused ? `${data.monitors.paused} paused` : undefined}
              delay={0.04}
            />
            <MetricCard
              label="Down"
              value={data.monitors.down}
              format="integer"
              icon={AlertTriangle}
              tone={data.monitors.down > 0 ? "down" : "default"}
              delay={0.08}
            />
            <MetricCard
              label="Unknown"
              value={data.monitors.unknown}
              format="integer"
              icon={CircleHelp}
              hint="No completed checks yet"
              delay={0.12}
            />
          </div>

          <div className="grid gap-4 sm:grid-cols-3">
            <MetricCard
              label="Uptime"
              value={data.uptime_percent}
              format="percent"
              icon={TrendingUp}
              delay={0.16}
              hint={data.uptime_percent == null ? "No checks in this window" : windowLabel}
            />
            <MetricCard
              label="Avg response time"
              value={data.avg_response_time_ms}
              format="ms"
              icon={Gauge}
              delay={0.2}
              hint={
                data.avg_response_time_ms == null
                  ? "No successful checks yet"
                  : "Successful checks only"
              }
            />
            <MetricCard
              label="Outages"
              value={data.incidents.outage_count}
              format="integer"
              icon={AlertTriangle}
              tone={data.incidents.outage_count > 0 ? "warning" : "default"}
              delay={0.24}
              hint={`${formatSeconds(data.incidents.downtime_seconds)} downtime · ${windowLabel}`}
            />
          </div>

          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Uptime — last 24h</CardTitle>
                <span className="text-muted-foreground font-mono text-xs">hourly buckets</span>
              </CardHeader>
              <CardContent>
                {data.uptime_series.length === 0 ? (
                  <p className="text-muted-foreground py-8 text-center text-sm">
                    No check activity in the last 24 hours.
                  </p>
                ) : (
                  <UptimeStrip buckets={data.uptime_series} />
                )}
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm font-medium">Response time</CardTitle>
              </CardHeader>
              <CardContent>
                <SeriesChart buckets={data.response_time_series} />
              </CardContent>
            </Card>
          </div>

          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Recent incidents</CardTitle>
                <div className="flex items-center gap-2">
                  <span className="text-muted-foreground font-mono text-xs">{windowLabel}</span>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="gap-1 text-xs"
                    onClick={() => navigate("/incidents")}
                  >
                    View all <ArrowRight className="size-3" />
                  </Button>
                </div>
              </CardHeader>
              <CardContent className="space-y-3">
                {data.recent_incidents.length === 0 ? (
                  <p className="text-muted-foreground py-6 text-center text-sm">
                    No incidents in this window — everything is running.
                  </p>
                ) : (
                  data.recent_incidents.slice(0, 5).map((incident) => (
                    <button
                      key={incident.id}
                      className="hover:bg-muted/60 flex w-full items-center justify-between gap-3 rounded-md px-2 py-1.5 text-left transition-colors"
                      onClick={() => navigate(`/monitors/${incident.monitor_id}`)}
                    >
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium">{incident.monitor_name}</p>
                        <p className="text-muted-foreground text-xs">
                          {formatRelative(incident.started_at)}
                          {incident.resolved_at ? "" : " · ongoing"}
                        </p>
                      </div>
                      <StatusPill
                        status={incident.status === "open" ? "down" : "up"}
                        className="shrink-0"
                      />
                    </button>
                  ))
                )}
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Recent checks</CardTitle>
                <div className="flex items-center gap-2">
                  <span className="text-muted-foreground font-mono text-xs">{windowLabel}</span>
                  <Timer className="text-muted-foreground size-4" />
                </div>
              </CardHeader>
              <CardContent className="space-y-3">
                {data.recent_checks.length === 0 ? (
                  <p className="text-muted-foreground py-6 text-center text-sm">
                    No checks in this window.
                  </p>
                ) : (
                  data.recent_checks.slice(0, 6).map((check, index) => (
                    <div
                      key={`${check.monitor_id}-${check.checked_at}-${index}`}
                      className="flex items-center justify-between gap-3 px-2"
                    >
                      <div className="min-w-0">
                        <p className="truncate text-sm">{check.monitor_name}</p>
                        <p className="text-muted-foreground font-mono text-xs">
                          {check.http_status_code
                            ? `HTTP ${check.http_status_code}`
                            : check.success
                              ? "OK"
                              : "failed"}{" "}
                          · {formatRelative(check.checked_at)}
                        </p>
                      </div>
                      <span
                        className={
                          check.success
                            ? "text-status-up font-mono text-xs"
                            : "text-status-down font-mono text-xs"
                        }
                      >
                        {formatMs(check.response_time_ms)}
                      </span>
                    </div>
                  ))
                )}
              </CardContent>
            </Card>
          </div>

          <p className="text-muted-foreground flex items-center gap-2 text-xs">
            <PauseCircle className="size-3.5" />
            Uptime is check-based (successful checks ÷ eligible checks). Paused periods are not
            excluded.
          </p>
        </div>
      )}
    </>
  );
}
