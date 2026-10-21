import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { MonitorsPage } from "@/pages/MonitorsPage";
import type { MonitorSummary, Paginated } from "@/types/api";

const mocks = vi.hoisted(() => ({
  useMonitors: vi.fn(),
}));

vi.mock("@/hooks/use-api", () => ({
  useMonitors: mocks.useMonitors,
  useDemoMode: () => false,
}));

vi.mock("@/features/monitors/monitor-form", () => ({
  MonitorFormDialog: () => null,
}));

vi.mock("@/features/monitors/monitor-actions", () => ({
  MonitorActions: () => <span>actions</span>,
}));

function monitor(overrides: Partial<MonitorSummary> = {}): MonitorSummary {
  return {
    id: "11111111-1111-1111-1111-111111111111",
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
    alert_webhook_configured: false,
    alert_email_configured: false,
    current_status: "up",
    consecutive_failures: 0,
    consecutive_successes: 3,
    created_at: "2026-10-01T00:00:00Z",
    updated_at: "2026-10-01T00:00:00Z",
    last_checked_at: "2026-10-01T00:00:00Z",
    next_check_at: "2026-10-01T00:01:00Z",
    latest_http_status: 200,
    latest_response_time_ms: 42,
    latest_success: true,
    uptime_percent: 99.9,
    checks_in_window: 100,
    degraded: false,
    ...overrides,
  };
}

function page(overrides: Partial<Paginated<MonitorSummary>> = {}): Paginated<MonitorSummary> {
  return {
    data: [monitor()],
    pagination: { page: 1, page_size: 20, total: 1, total_pages: 1 },
    ...overrides,
  };
}

function renderPage() {
  return render(
    <MemoryRouter>
      <MonitorsPage />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.useMonitors.mockReset();
  mocks.useMonitors.mockReturnValue({
    data: page(),
    isLoading: false,
    isError: false,
    refetch: vi.fn(),
  });
});

describe("MonitorsPage", () => {
  it("renders monitor rows with status, uptime and actions", () => {
    renderPage();
    expect(screen.getAllByText("API").length).toBeGreaterThan(0);
    expect(screen.getAllByText("99.9%").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Up").length).toBeGreaterThan(0);
    expect(screen.getAllByText("actions").length).toBeGreaterThan(0);
  });

  it("shows a loading skeleton before data arrives", () => {
    mocks.useMonitors.mockReturnValue({
      data: undefined,
      isLoading: true,
      isError: false,
      refetch: vi.fn(),
    });
    const { container } = renderPage();
    expect(container.querySelectorAll('[data-slot="skeleton"]').length).toBeGreaterThan(0);
  });

  it("renders the retryable error state", async () => {
    const refetch = vi.fn();
    mocks.useMonitors.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      refetch,
    });
    renderPage();
    expect(screen.getByText("Couldn't load monitors")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(refetch).toHaveBeenCalledTimes(1);
  });

  it("shows the teaching empty state for a new installation", () => {
    mocks.useMonitors.mockReturnValue({
      data: page({ data: [], pagination: { page: 1, page_size: 20, total: 0, total_pages: 0 } }),
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });
    renderPage();
    expect(screen.getByText("No monitors yet")).toBeInTheDocument();
    expect(screen.getByText(/Add a URL and PulseWatch starts checking/)).toBeInTheDocument();
  });

  it("debounces search into the list query", async () => {
    renderPage();
    await userEvent.type(screen.getByLabelText("Search monitors"), "api");

    await waitFor(() => {
      const lastCall = mocks.useMonitors.mock.calls.at(-1)?.[0];
      expect(lastCall).toMatchObject({ search: "api", page: 1 });
    });
  });

  it("passes the default sort to the API and exposes sort controls", () => {
    renderPage();
    expect(screen.getByLabelText("Sort by")).toBeInTheDocument();
    expect(mocks.useMonitors).toHaveBeenCalledWith(
      expect.objectContaining({ sort: "created", order: "desc" }),
    );
  });

  it("paginates through the result set", async () => {
    mocks.useMonitors.mockReturnValue({
      data: page({
        pagination: { page: 1, page_size: 20, total: 40, total_pages: 2 },
      }),
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });
    renderPage();
    await userEvent.click(screen.getByRole("button", { name: "Next" }));

    await waitFor(() => {
      const lastCall = mocks.useMonitors.mock.calls.at(-1)?.[0];
      expect(lastCall).toMatchObject({ page: 2 });
    });
  });
});
