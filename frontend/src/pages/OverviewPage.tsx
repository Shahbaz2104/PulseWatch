import { useNavigate } from "react-router-dom";
import {
  Activity,
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  CircleHelp,
  Gauge,
  PauseCircle,
  Plus,
  Timer,
  TrendingUp,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/empty-state";
import { MetricCard } from "@/components/metric-card";
import { PageHeader } from "@/components/page-header";
import { StatusPill } from "@/components/status-pill";
import { UptimeStrip } from "@/components/uptime-strip";
import { SeriesChart } from "@/components/series-chart";
import { useOverview } from "@/hooks/use-api";
import { formatMs, formatRelative } from "@/lib/format";

export function OverviewPage() {
  const navigate = useNavigate();
  const { data, isLoading, isError, refetch, isFetching } = useOverview();

  const hasMonitors = (data?.monitors.total ?? 0) > 0;

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
          icon={Activity}
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
              icon={Activity}
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

          <div className="grid gap-4 sm:grid-cols-2">
            <MetricCard
              label="Uptime"
              value={data.uptime_percent}
              format="percent"
              icon={TrendingUp}
              delay={0.16}
              hint={data.uptime_percent == null ? "No checks in this window" : "Last 24 hours"}
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
          </div>

          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Uptime — last 24h</CardTitle>
                <span className="font-mono text-xs text-muted-foreground">
                  {data.incidents.outage_count} outage{data.incidents.outage_count === 1 ? "" : "s"}
                  {" · "}
                  {data.incidents.downtime_seconds > 0
                    ? `${Math.round(data.incidents.downtime_seconds / 60)}m downtime`
                    : "no downtime"}
                </span>
              </CardHeader>
              <CardContent>
                {data.uptime_series.length === 0 ? (
                  <p className="py-8 text-center text-sm text-muted-foreground">
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
                <Button
                  variant="ghost"
                  size="sm"
                  className="gap-1 text-xs"
                  onClick={() => navigate("/incidents")}
                >
                  View all <ArrowRight className="size-3" />
                </Button>
              </CardHeader>
              <CardContent className="space-y-3">
                {data.recent_incidents.length === 0 ? (
                  <p className="py-6 text-center text-sm text-muted-foreground">
                    No incidents recorded — everything is running.
                  </p>
                ) : (
                  data.recent_incidents.slice(0, 5).map((incident) => (
                    <button
                      key={incident.id}
                      className="flex w-full items-center justify-between gap-3 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-muted/60"
                      onClick={() => navigate(`/monitors/${incident.monitor_id}`)}
                    >
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium">{incident.monitor_name}</p>
                        <p className="text-xs text-muted-foreground">
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
                <Timer className="size-4 text-muted-foreground" />
              </CardHeader>
              <CardContent className="space-y-3">
                {data.recent_checks.length === 0 ? (
                  <p className="py-6 text-center text-sm text-muted-foreground">
                    No checks have run yet.
                  </p>
                ) : (
                  data.recent_checks.slice(0, 6).map((check, index) => (
                    <div
                      key={`${check.monitor_id}-${check.checked_at}-${index}`}
                      className="flex items-center justify-between gap-3 px-2"
                    >
                      <div className="min-w-0">
                        <p className="truncate text-sm">{check.monitor_name}</p>
                        <p className="font-mono text-xs text-muted-foreground">
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
                            ? "font-mono text-xs text-status-up"
                            : "font-mono text-xs text-status-down"
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

          <p className="flex items-center gap-2 text-xs text-muted-foreground">
            <PauseCircle className="size-3.5" />
            Uptime is check-based (successful checks ÷ eligible checks). Paused periods are not
            excluded.
          </p>
        </div>
      )}
    </>
  );
}
