import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  Heartbeat,
  Plus,
  MagnifyingGlass as Search,
  SortAscending,
  SortDescending,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { EmptyState } from "@/components/empty-state";
import { PageHeader } from "@/components/page-header";
import { StatusPill } from "@/components/status-pill";
import { WindowPicker } from "@/components/window-picker";
import { Skeleton } from "@/components/ui/skeleton";
import { MonitorActions } from "@/features/monitors/monitor-actions";
import { MonitorFormDialog } from "@/features/monitors/monitor-form";
import { useDemoMode, useMonitors } from "@/hooks/use-api";
import { formatMs, formatPercent, formatRelative } from "@/lib/format";
import type { MonitorSummary, WindowQuery } from "@/types/api";

const STATUS_OPTIONS = [
  { value: "all", label: "All statuses" },
  { value: "up", label: "Up" },
  { value: "down", label: "Down" },
  { value: "unknown", label: "Unknown" },
  { value: "paused", label: "Paused" },
] as const;

const SORT_OPTIONS = [
  { value: "created", label: "Newest" },
  { value: "name", label: "Name" },
  { value: "uptime", label: "Uptime" },
  { value: "response_time", label: "Response time" },
  { value: "last_checked", label: "Last checked" },
] as const;

type SortField = (typeof SORT_OPTIONS)[number]["value"];
type SortOrder = "asc" | "desc";

function MonitorRow({ monitor, onEdit }: { monitor: MonitorSummary; onEdit: () => void }) {
  return (
    <TableRow className="cursor-pointer" onClick={onEdit}>
      <TableCell onClick={(event) => event.stopPropagation()}>
        <a
          href={`/monitors/${monitor.id}`}
          className="hover:text-primary font-medium hover:underline"
        >
          {monitor.name}
        </a>
        <p className="text-muted-foreground truncate font-mono text-xs">{monitor.url}</p>
      </TableCell>
      <TableCell>
        <StatusPill status={monitor.current_status} degraded={monitor.degraded} />
      </TableCell>
      <TableCell className="font-mono text-sm">{monitor.latest_http_status ?? "—"}</TableCell>
      <TableCell className="font-mono text-sm">
        {formatMs(monitor.latest_response_time_ms)}
      </TableCell>
      <TableCell className="font-mono text-sm">{formatPercent(monitor.uptime_percent)}</TableCell>
      <TableCell className="font-mono text-sm">{monitor.check_interval_seconds}s</TableCell>
      <TableCell className="text-muted-foreground text-sm">
        {formatRelative(monitor.last_checked_at)}
      </TableCell>
      <TableCell className="text-muted-foreground text-sm">
        {formatRelative(monitor.next_check_at)}
      </TableCell>
      <TableCell onClick={(event) => event.stopPropagation()}>
        <MonitorActions monitor={monitor} onEdit={onEdit} />
      </TableCell>
    </TableRow>
  );
}

function MonitorCard({ monitor, onEdit }: { monitor: MonitorSummary; onEdit: () => void }) {
  return (
    <Card className="py-4" onClick={onEdit}>
      <CardContent className="gap-2 px-4">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0">
            <a
              href={`/monitors/${monitor.id}`}
              className="hover:text-primary font-medium hover:underline"
              onClick={(event) => event.stopPropagation()}
            >
              {monitor.name}
            </a>
            <p className="text-muted-foreground truncate font-mono text-xs">{monitor.url}</p>
          </div>
          <div onClick={(event) => event.stopPropagation()}>
            <MonitorActions monitor={monitor} onEdit={onEdit} />
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm">
          <StatusPill status={monitor.current_status} degraded={monitor.degraded} />
          <span className="text-muted-foreground font-mono">
            {formatMs(monitor.latest_response_time_ms)}
          </span>
          <span className="text-muted-foreground font-mono">
            {formatPercent(monitor.uptime_percent)} uptime
          </span>
          <span className="text-muted-foreground font-mono">
            every {monitor.check_interval_seconds}s
          </span>
        </div>
      </CardContent>
    </Card>
  );
}

export function MonitorsPage() {
  const [params, setParams] = useSearchParams();
  const demoMode = useDemoMode();
  const [search, setSearch] = useState(params.get("search") ?? "");
  const [status, setStatus] = useState(params.get("status") ?? "all");
  const [sort, setSort] = useState<SortField>("created");
  const [order, setOrder] = useState<SortOrder>("desc");
  const [windowQuery, setWindowQuery] = useState<WindowQuery>({ window: "24h" });
  const [page, setPage] = useState(1);
  const [dialogOpen, setDialogOpen] = useState(params.get("new") === "1");
  const [editing, setEditing] = useState<MonitorSummary | null>(null);

  // Keep the ?new=1 flag in sync so deep links open the create dialog once.
  useEffect(() => {
    if (dialogOpen && params.get("new") === "1") {
      const next = new URLSearchParams(params);
      next.delete("new");
      setParams(next, { replace: true });
    }
  }, [dialogOpen, params, setParams]);

  // The create dialog must never open in a read-only demo, even via ?new=1.
  useEffect(() => {
    if (demoMode) {
      setDialogOpen(false);
    }
  }, [demoMode]);

  // Debounce search into the query.
  const [debouncedSearch, setDebouncedSearch] = useState(search);
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search), 250);
    return () => clearTimeout(timer);
  }, [search]);

  const query = useMemo(
    () => ({
      search: debouncedSearch || undefined,
      status: status === "all" ? undefined : status,
      sort,
      order,
      window: windowQuery.window,
      from: windowQuery.from,
      to: windowQuery.to,
      page,
      page_size: 20,
    }),
    [debouncedSearch, status, sort, order, windowQuery, page],
  );

  const { data, isLoading, isError, refetch } = useMonitors(query);

  const openCreate = () => {
    setEditing(null);
    setDialogOpen(true);
  };
  const openEdit = (monitor: MonitorSummary) => {
    setEditing(monitor);
    setDialogOpen(true);
  };

  return (
    <>
      <PageHeader
        title="Monitors"
        description="Every watched URL, its live status, and controls."
        actions={
          <Button
            onClick={openCreate}
            disabled={demoMode}
            title={demoMode ? "Read-only demo: changes are blocked" : undefined}
          >
            <Plus /> Add monitor
          </Button>
        }
      />

      <div className="mb-4 flex flex-wrap items-center gap-2">
        <div className="relative min-w-48 flex-1">
          <Search className="text-muted-foreground absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
          <Input
            aria-label="Search monitors"
            placeholder="Search by name or URL…"
            className="pl-8"
            value={search}
            onChange={(event) => {
              setSearch(event.target.value);
              setPage(1);
            }}
          />
        </div>
        <Select
          value={status}
          onValueChange={(value) => {
            setStatus(value);
            setPage(1);
          }}
        >
          <SelectTrigger className="w-36" aria-label="Filter by status">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {STATUS_OPTIONS.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select
          value={sort}
          onValueChange={(value) => {
            setSort(value as SortField);
            setOrder(value === "name" ? "asc" : "desc");
            setPage(1);
          }}
        >
          <SelectTrigger className="w-40" aria-label="Sort by">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {SORT_OPTIONS.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Button
          variant="outline"
          size="icon"
          aria-label={`Sort ${order === "asc" ? "descending" : "ascending"}`}
          title={order === "asc" ? "Ascending" : "Descending"}
          onClick={() => {
            setOrder((value) => (value === "asc" ? "desc" : "asc"));
            setPage(1);
          }}
        >
          {order === "asc" ? <SortAscending /> : <SortDescending />}
        </Button>
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
          {Array.from({ length: 5 }).map((_, index) => (
            <Skeleton key={index} className="h-14" />
          ))}
        </div>
      )}

      {isError && (
        <EmptyState
          icon={Heartbeat}
          title="Couldn't load monitors"
          description="The API didn't respond. Is the backend running?"
          action={{ label: "Retry", onClick: () => void refetch() }}
        />
      )}

      {data && data.data.length === 0 && (
        <EmptyState
          icon={Heartbeat}
          title={debouncedSearch || status !== "all" ? "No monitors match" : "No monitors yet"}
          description={
            debouncedSearch || status !== "all"
              ? "Try clearing the search or changing the status filter."
              : "Add a URL and PulseWatch starts checking it within seconds."
          }
          action={
            debouncedSearch || status !== "all"
              ? {
                  label: "Clear filters",
                  onClick: () => {
                    setSearch("");
                    setStatus("all");
                    setPage(1);
                  },
                }
              : { label: "Add monitor", onClick: openCreate }
          }
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
                  <TableHead>Status</TableHead>
                  <TableHead>HTTP</TableHead>
                  <TableHead>Response</TableHead>
                  <TableHead>Uptime</TableHead>
                  <TableHead>Interval</TableHead>
                  <TableHead>Last check</TableHead>
                  <TableHead>Next check</TableHead>
                  <TableHead className="w-10" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {data.data.map((monitor) => (
                  <MonitorRow key={monitor.id} monitor={monitor} onEdit={() => openEdit(monitor)} />
                ))}
              </TableBody>
            </Table>
          </div>

          {/* Mobile cards */}
          <div className="space-y-3 md:hidden">
            {data.data.map((monitor) => (
              <MonitorCard key={monitor.id} monitor={monitor} onEdit={() => openEdit(monitor)} />
            ))}
          </div>

          {data.pagination.total_pages > 1 && (
            <div className="text-muted-foreground mt-4 flex items-center justify-between text-sm">
              <span>
                Page {data.pagination.page} of {data.pagination.total_pages} ·{" "}
                {data.pagination.total} monitor{data.pagination.total === 1 ? "" : "s"}
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

      <MonitorFormDialog
        open={dialogOpen}
        onOpenChange={(open) => {
          setDialogOpen(open);
          if (!open) setEditing(null);
        }}
        editing={editing}
      />
    </>
  );
}
