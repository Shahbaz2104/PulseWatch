import { useState } from "react";
import { Link } from "react-router-dom";
import { AlertTriangle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { EmptyState } from "@/components/empty-state";
import { PageHeader } from "@/components/page-header";
import { StatusPill } from "@/components/status-pill";
import { WindowPicker } from "@/components/window-picker";
import { useIncidents } from "@/hooks/use-api";
import { formatClock, formatSeconds } from "@/lib/format";
import type { WindowQuery } from "@/types/api";

export function IncidentsPage() {
  const [status, setStatus] = useState<"all" | "open" | "resolved">("all");
  const [windowQuery, setWindowQuery] = useState<WindowQuery>({ window: "30d" });
  const [page, setPage] = useState(1);

  const { data, isLoading, isError, refetch } = useIncidents({
    status: status === "all" ? undefined : status,
    window: windowQuery.window,
    from: windowQuery.from,
    to: windowQuery.to,
    page,
    page_size: 25,
  });

  return (
    <>
      <PageHeader
        title="Incidents"
        description="Outages from up→down transitions. Retries never create incidents."
      />

      <div className="mb-4 flex flex-wrap items-center gap-2">
        <Select
          value={status}
          onValueChange={(value) => {
            setStatus(value as typeof status);
            setPage(1);
          }}
        >
          <SelectTrigger className="w-36" aria-label="Filter by status">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All incidents</SelectItem>
            <SelectItem value="open">Open</SelectItem>
            <SelectItem value="resolved">Resolved</SelectItem>
          </SelectContent>
        </Select>
        <WindowPicker
          value={windowQuery}
          onChange={(next) => {
            setWindowQuery(next);
            setPage(1);
          }}
        />
      </div>

      {isLoading && (
        <div className="space-y-2">
          {Array.from({ length: 4 }).map((_, index) => (
            <Skeleton key={index} className="h-14" />
          ))}
        </div>
      )}

      {isError && (
        <EmptyState
          icon={AlertTriangle}
          title="Couldn't load incidents"
          description="The API didn't respond."
          action={{ label: "Retry", onClick: () => void refetch() }}
        />
      )}

      {data && data.data.length === 0 && (
        <EmptyState
          icon={AlertTriangle}
          title="No incidents in this window"
          description="Nothing went down during the selected period. Incidents appear here the moment a monitor crosses its failure threshold."
        />
      )}

      {data && data.data.length > 0 && (
        <>
          {/* Desktop table */}
          <div className="hidden overflow-hidden rounded-lg border md:block">
            <Table>
              <TableHeader>
                <TableRow className="bg-muted/40 hover:bg-muted/40">
                  <TableHead>Monitor</TableHead>
                  <TableHead>State</TableHead>
                  <TableHead>Started</TableHead>
                  <TableHead>Resolved</TableHead>
                  <TableHead>Duration</TableHead>
                  <TableHead>Reason</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.data.map((incident) => (
                  <TableRow key={incident.id}>
                    <TableCell>
                      <Link
                        to={`/monitors/${incident.monitor_id}`}
                        className="font-medium hover:text-primary hover:underline"
                      >
                        {incident.monitor_name}
                      </Link>
                      <p className="truncate font-mono text-xs text-muted-foreground">
                        {incident.monitor_url}
                      </p>
                    </TableCell>
                    <TableCell>
                      <StatusPill
                        status={incident.status === "open" ? "down" : "up"}
                      />
                    </TableCell>
                    <TableCell className="font-mono text-xs whitespace-nowrap">
                      {formatClock(incident.started_at)}
                    </TableCell>
                    <TableCell className="font-mono text-xs whitespace-nowrap">
                      {incident.resolved_at ? formatClock(incident.resolved_at) : "—"}
                    </TableCell>
                    <TableCell className="font-mono text-sm">
                      {formatSeconds(incident.duration_seconds)}
                    </TableCell>
                    <TableCell className="max-w-52 truncate text-xs text-muted-foreground">
                      {incident.failure_reason ?? "—"}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>

          {/* Mobile cards */}
          <div className="space-y-3 md:hidden">
            {data.data.map((incident) => (
              <Card key={incident.id} className="py-4">
                <CardContent className="gap-2 px-4">
                  <div className="flex items-center justify-between gap-2">
                    <Link
                      to={`/monitors/${incident.monitor_id}`}
                      className="font-medium hover:text-primary hover:underline"
                    >
                      {incident.monitor_name}
                    </Link>
                    <StatusPill status={incident.status === "open" ? "down" : "up"} />
                  </div>
                  <p className="font-mono text-xs text-muted-foreground">
                    {formatClock(incident.started_at)} →{" "}
                    {incident.resolved_at ? formatClock(incident.resolved_at) : "ongoing"}
                  </p>
                  <div className="flex items-center gap-2 text-sm">
                    <Badge variant="secondary" className="font-mono">
                      {formatSeconds(incident.duration_seconds)}
                    </Badge>
                    {incident.failure_reason && (
                      <span className="truncate text-xs text-muted-foreground">
                        {incident.failure_reason}
                      </span>
                    )}
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>

          {data.pagination.total_pages > 1 && (
            <div className="mt-4 flex items-center justify-between text-sm text-muted-foreground">
              <span>
                Page {data.pagination.page} of {data.pagination.total_pages} ·{" "}
                {data.pagination.total} incident{data.pagination.total === 1 ? "" : "s"}
              </span>
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={page <= 1}
                  onClick={() => setPage((value) => Math.max(1, value - 1))}
                >
                  Previous
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={page >= data.pagination.total_pages}
                  onClick={() => setPage((value) => value + 1)}
                >
                  Next
                </Button>
              </div>
            </div>
          )}
        </>
      )}
    </>
  );
}
