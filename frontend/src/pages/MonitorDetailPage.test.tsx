import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { MonitorDetailPage } from "@/pages/MonitorDetailPage";
import type {
  CheckResult,
  CheckSummaryStats,
  IncidentWithMonitor,
  MonitorSummary,
  Paginated,
} from "@/types/api";

const mocks = vi.hoisted(() => ({
  useMonitor: vi.fn(),
  useCheckSummary: vi.fn(),
  useChecks: vi.fn(),
  useIncidents: vi.fn(),
  useResponseTimes: vi.fn(),
  useCheckNow: vi.fn(),
  usePauseMonitor: vi.fn(),
  useResumeMonitor: vi.fn(),
}));

vi.mock("@/hooks/use-api", () => ({
  useMonitor: mocks.useMonitor,
  useCheckSummary: mocks.useCheckSummary,
  useChecks: mocks.useChecks,
  useIncidents: mocks.useIncidents,
  useResponseTimes: mocks.useResponseTimes,
  useCheckNow: mocks.useCheckNow,
  usePauseMonitor: mocks.usePauseMonitor,
  useResumeMonitor: mocks.useResumeMonitor,
}));

vi.mock("@/components/series-chart", () => ({
  SeriesChart: () => <div>chart</div>,
}));

vi.mock("@/components/uptime-strip", () => ({
  UptimeStrip: () => <div>strip</div>,
}));

vi.mock("@/features/monitors/monitor-form", () => ({
  MonitorFormDialog: () => null,
}));

const MONITOR_ID = "11111111-1111-1111-1111-111111111111";

function monitor(): MonitorSummary {
  return {
    id: MONITOR_ID,
    name: "API",
    url: "https://api.example.com/health",
    enabled: true,
    check_interval_seconds: 60,
    timeout_ms: 5000,
    expected_status_min: 200,
    expected_status_max: 399,
    consecutive_failures_threshold: 1,
    consecutive_successes_threshold: 1,
    retry_max_attempts: 2,
    current_status: "up",
    consecutive_failures: 0,
    consecutive_successes: 2,
    created_at: "2026-10-01T00:00:00Z",
    updated_at: "2026-10-01T00:00:00Z",
    last_checked_at: "2026-10-20T10:00:00Z",
    next_check_at: "2026-10-20T10:01:00Z",
    latest_http_status: 200,
    latest_response_time_ms: 42,
    latest_success: true,
    uptime_percent: 99.9,
    checks_in_window: 100,
    degraded: false,
  };
}

function summary(overrides: Partial<CheckSummaryStats> = {}): CheckSummaryStats {
  return {
    total_checks: 10,
    successful_checks: 9,
    uptime_percent: 90,
    latest_response_time_ms: 42,
    avg_response_time_ms: 50,
    median_response_time_ms: 45,
    p95_response_time_ms: 90,
    window_start: "2026-10-19T00:00:00Z",
    window_end: "2026-10-20T00:00:00Z",
    ...overrides,
  };
}

function check(overrides: Partial<CheckResult> = {}): CheckResult {
  return {
    id: "check-1",
    monitor_id: MONITOR_ID,
    checked_at: "2026-10-20T10:00:00Z",
    completed_at: "2026-10-20T10:00:00Z",
    http_status_code: 500,
    response_time_ms: 120,
    success: false,
    error_type: "http_status_mismatch",
    error_message: "HTTP 500 outside expected range",
    attempt_count: 2,
    expected_status_match: false,
    ...overrides,
  };
}

function incident(): IncidentWithMonitor {
  return {
    id: "incident-1",
    monitor_id: MONITOR_ID,
    started_at: "2026-10-19T10:00:00Z",
    resolved_at: "2026-10-19T10:10:00Z",
    status: "resolved",
    failure_reason: "connection refused",
    created_at: "2026-10-19T10:00:00Z",
    updated_at: "2026-10-19T10:10:00Z",
    monitor_name: "API",
    monitor_url: "https://api.example.com/health",
    duration_seconds: 600,
  };
}

function history(rows: CheckResult[]): Paginated<CheckResult> {
  return {
    data: rows,
    pagination: { page: 1, page_size: 25, total: rows.length, total_pages: 1 },
  };
}

function renderPage() {
  return render(
    <MemoryRouter initialEntries={[`/monitors/${MONITOR_ID}`]}>
      <Routes>
        <Route path="/monitors/:monitorId" element={<MonitorDetailPage />} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.useMonitor.mockReset().mockReturnValue({
    data: monitor(),
    isLoading: false,
    isError: false,
  });
  mocks.useCheckSummary.mockReset().mockReturnValue({ data: summary(), isLoading: false });
  mocks.useChecks
    .mockReset()
    .mockImplementation((_id: string, query: { result?: string }) =>
      query.result === "failure"
        ? { data: history([check()]), isLoading: false }
        : { data: history([check({ success: true, http_status_code: 200 })]), isLoading: false },
    );
  mocks.useIncidents.mockReset().mockReturnValue({
    data: {
      data: [incident()],
      pagination: { page: 1, page_size: 5, total: 1, total_pages: 1 },
    },
    isLoading: false,
  });
  mocks.useResponseTimes.mockReset().mockReturnValue({
    data: { bucket: "hour", series: [], monitors: [], window: {} },
    isLoading: false,
  });
  const mutation = { mutate: vi.fn(), isPending: false };
  mocks.useCheckNow.mockReturnValue(mutation);
  mocks.usePauseMonitor.mockReturnValue(mutation);
  mocks.useResumeMonitor.mockReturnValue(mutation);
});

describe("MonitorDetailPage", () => {
  it("renders windowed uptime, latest response and latest HTTP status", () => {
    renderPage();
    expect(screen.getByText("Uptime 24h")).toBeInTheDocument();
    expect(screen.getByText("Uptime 7d")).toBeInTheDocument();
    expect(screen.getByText("Uptime 30d")).toBeInTheDocument();
    expect(screen.getByLabelText("42 milliseconds")).toBeInTheDocument();
    expect(screen.getByLabelText("200")).toBeInTheDocument();
    expect(screen.getByText("Within expected range")).toBeInTheDocument();
  });

  it("renders incident history with duration and failure reason", () => {
    renderPage();
    expect(screen.getByText("Incident history — 30d")).toBeInTheDocument();
    expect(screen.getByText("connection refused")).toBeInTheDocument();
    expect(screen.getByText("10m")).toBeInTheDocument();
  });

  it("renders recent errors with error type and message", () => {
    renderPage();
    expect(screen.getByText("Recent errors — 30d")).toBeInTheDocument();
    expect(screen.getByText("http_status_mismatch")).toBeInTheDocument();
    expect(screen.getAllByText("HTTP 500 outside expected range").length).toBeGreaterThan(0);
  });

  it("shows honest empty states when there are no incidents or failures", () => {
    mocks.useIncidents.mockReturnValue({
      data: {
        data: [],
        pagination: { page: 1, page_size: 5, total: 0, total_pages: 0 },
      },
      isLoading: false,
    });
    mocks.useChecks.mockImplementation((_id: string, query: { result?: string }) => ({
      data: history(query.result === "failure" ? [] : [check({ success: true })]),
      isLoading: false,
    }));
    renderPage();
    expect(screen.getByText("No incidents in the last 30 days.")).toBeInTheDocument();
    expect(screen.getByText("No failed checks in the last 30 days.")).toBeInTheDocument();
  });
});
