import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { OverviewPage } from "@/pages/OverviewPage";
import type { OverviewReport } from "@/types/api";

const mocks = vi.hoisted(() => ({
  useOverview: vi.fn(),
}));

vi.mock("@/hooks/use-api", () => ({
  useOverview: mocks.useOverview,
}));

vi.mock("@/components/series-chart", () => ({
  SeriesChart: () => <div>chart</div>,
}));

vi.mock("@/components/uptime-strip", () => ({
  UptimeStrip: () => <div>strip</div>,
}));

function overview(overrides: Partial<OverviewReport> = {}): OverviewReport {
  return {
    window: { start: "2026-10-19T00:00:00Z", end: "2026-10-20T00:00:00Z" },
    monitors: { total: 3, up: 2, down: 1, unknown: 0, paused: 1 },
    uptime_percent: 99.5,
    avg_response_time_ms: 120,
    incidents: { outage_count: 1, downtime_seconds: 3600 },
    recent_incidents: [
      {
        id: "incident-1",
        monitor_id: "monitor-1",
        started_at: "2026-10-19T10:00:00Z",
        resolved_at: "2026-10-19T11:00:00Z",
        status: "resolved",
        failure_reason: "timeout",
        created_at: "2026-10-19T10:00:00Z",
        updated_at: "2026-10-19T11:00:00Z",
        monitor_name: "API",
        monitor_url: "https://api.example.com",
        duration_seconds: 3600,
      },
    ],
    recent_checks: [
      {
        monitor_id: "monitor-1",
        monitor_name: "API",
        checked_at: "2026-10-20T00:00:00Z",
        success: true,
        http_status_code: 200,
        response_time_ms: 42,
      },
    ],
    uptime_series: [],
    response_time_series: [],
    ...overrides,
  };
}

function renderPage() {
  return render(
    <MemoryRouter>
      <OverviewPage />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.useOverview.mockReset().mockReturnValue({
    data: overview(),
    isLoading: false,
    isError: false,
    isFetching: false,
    refetch: vi.fn(),
  });
});

describe("OverviewPage", () => {
  it("renders census, selected-window uptime, latency and outage metrics", () => {
    renderPage();
    expect(screen.getByText("Total monitors")).toBeInTheDocument();
    expect(screen.getByText("Operational")).toBeInTheDocument();
    expect(screen.getByText("Down")).toBeInTheDocument();
    expect(screen.getByText("Unknown")).toBeInTheDocument();
    expect(screen.getByLabelText("99.50 percent")).toBeInTheDocument();
    expect(screen.getByLabelText("120 milliseconds")).toBeInTheDocument();
    expect(screen.getByText("1h downtime · last 24h")).toBeInTheDocument();
  });

  it("renders recent incidents and recent checks from the API", () => {
    renderPage();
    expect(screen.getByText("Recent incidents")).toBeInTheDocument();
    expect(screen.getAllByText("API").length).toBeGreaterThan(0);
    expect(screen.getByText("Recent checks")).toBeInTheDocument();
    expect(screen.getByText(/HTTP 200/)).toBeInTheDocument();
  });

  it("shows the teaching empty state when no monitors exist", () => {
    mocks.useOverview.mockReturnValue({
      data: overview({
        monitors: { total: 0, up: 0, down: 0, unknown: 0, paused: 0 },
        uptime_percent: null,
        avg_response_time_ms: null,
        incidents: { outage_count: 0, downtime_seconds: 0 },
        recent_incidents: [],
        recent_checks: [],
      }),
      isLoading: false,
      isError: false,
      isFetching: false,
      refetch: vi.fn(),
    });
    renderPage();
    expect(screen.getByText("No monitors yet")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add your first monitor" })).toBeInTheDocument();
  });

  it("refetches with the newly selected reporting window", async () => {
    renderPage();
    expect(mocks.useOverview).toHaveBeenCalledWith({ window: "24h" });

    await userEvent.click(screen.getByRole("tab", { name: "7d" }));

    await waitFor(() => {
      expect(mocks.useOverview.mock.calls.at(-1)?.[0]).toEqual({ window: "7d" });
    });
    expect(screen.getAllByText(/last 7d/).length).toBeGreaterThan(0);
  });

  it("shows a retryable error state", async () => {
    const refetch = vi.fn();
    mocks.useOverview.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      isFetching: false,
      refetch,
    });
    renderPage();
    expect(screen.getByText("Couldn't load the overview")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(refetch).toHaveBeenCalledTimes(1);
  });
});
