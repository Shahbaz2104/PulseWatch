import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { MonitorActions } from "@/features/monitors/monitor-actions";
import type { MonitorSummary } from "@/types/api";

const mocks = vi.hoisted(() => ({
  useCheckNow: vi.fn(),
  useDeleteMonitor: vi.fn(),
  usePauseMonitor: vi.fn(),
  useResumeMonitor: vi.fn(),
}));

vi.mock("@/hooks/use-api", () => ({
  useCheckNow: mocks.useCheckNow,
  useDeleteMonitor: mocks.useDeleteMonitor,
  usePauseMonitor: mocks.usePauseMonitor,
  useResumeMonitor: mocks.useResumeMonitor,
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
    ...overrides,
  };
}

function renderActions(value: MonitorSummary = monitor()) {
  return render(
    <MemoryRouter>
      <MonitorActions monitor={value} onEdit={vi.fn()} />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.useCheckNow.mockReset().mockReturnValue({ mutate: vi.fn(), isPending: false });
  mocks.useDeleteMonitor.mockReset().mockReturnValue({ mutate: vi.fn(), isPending: false });
  mocks.usePauseMonitor.mockReset().mockReturnValue({ mutate: vi.fn(), isPending: false });
  mocks.useResumeMonitor.mockReset().mockReturnValue({ mutate: vi.fn(), isPending: false });
});

describe("MonitorActions", () => {
  it("pauses a running monitor", async () => {
    renderActions();
    await userEvent.click(screen.getByRole("button", { name: "Actions for API" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Pause" }));

    await waitFor(() => {
      expect(mocks.usePauseMonitor.mock.results[0]?.value.mutate).toHaveBeenCalledWith(
        monitor().id,
        expect.objectContaining({ onSuccess: expect.any(Function) }),
      );
    });
  });

  it("resumes a paused monitor and disables check now", async () => {
    renderActions(monitor({ current_status: "paused" }));
    await userEvent.click(screen.getByRole("button", { name: "Actions for API" }));
    expect(await screen.findByRole("menuitem", { name: "Check now" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await userEvent.click(screen.getByRole("menuitem", { name: "Resume" }));

    await waitFor(() => {
      expect(mocks.useResumeMonitor.mock.results[0]?.value.mutate).toHaveBeenCalledWith(
        monitor().id,
        expect.objectContaining({ onSuccess: expect.any(Function) }),
      );
    });
  });

  it("requires confirmation before deleting", async () => {
    renderActions();
    await userEvent.click(screen.getByRole("button", { name: "Actions for API" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));

    expect(await screen.findByText(/Delete “API”\?/)).toBeInTheDocument();
    expect(mocks.useDeleteMonitor.mock.results[0]?.value.mutate).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Delete monitor" }));
    await waitFor(() => {
      expect(mocks.useDeleteMonitor.mock.results[0]?.value.mutate).toHaveBeenCalledWith(
        monitor().id,
        expect.objectContaining({ onSuccess: expect.any(Function) }),
      );
    });
  });
});
