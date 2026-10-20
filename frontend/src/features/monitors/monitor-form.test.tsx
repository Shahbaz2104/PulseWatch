import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MonitorFormDialog } from "@/features/monitors/monitor-form";
import type { AppSettings, MonitorSummary } from "@/types/api";

const mocks = vi.hoisted(() => ({
  useSettings: vi.fn(),
  useCreateMonitor: vi.fn(),
  useUpdateMonitor: vi.fn(),
  create: vi.fn(),
  update: vi.fn(),
}));

vi.mock("@/hooks/use-api", () => ({
  useSettings: mocks.useSettings,
  useCreateMonitor: mocks.useCreateMonitor,
  useUpdateMonitor: mocks.useUpdateMonitor,
}));

const SETTINGS: AppSettings = {
  default_check_interval_seconds: 30,
  default_timeout_ms: 5000,
  default_retry_max_attempts: 2,
  default_retry_backoff_ms: 500,
  default_retry_backoff_max_ms: 10_000,
  default_expected_status_min: 200,
  default_expected_status_max: 399,
  max_concurrent_checks: 16,
};

function editingMonitor(): MonitorSummary {
  return {
    id: "11111111-1111-1111-1111-111111111111",
    name: "Existing",
    url: "https://existing.example.com",
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
    consecutive_successes: 1,
    created_at: "2026-10-01T00:00:00Z",
    updated_at: "2026-10-01T00:00:00Z",
    last_checked_at: null,
    next_check_at: null,
    latest_http_status: null,
    latest_response_time_ms: null,
    latest_success: null,
    uptime_percent: null,
    checks_in_window: 0,
    degraded: false,
  };
}

function renderDialog(editing?: MonitorSummary) {
  const onOpenChange = vi.fn();
  render(<MonitorFormDialog open onOpenChange={onOpenChange} editing={editing} />);
  return { onOpenChange };
}

beforeEach(() => {
  mocks.create.mockReset();
  mocks.update.mockReset();
  mocks.useSettings.mockReset().mockReturnValue({ data: SETTINGS, isLoading: false });
  mocks.useCreateMonitor.mockReset().mockReturnValue({
    mutate: mocks.create,
    isPending: false,
    reset: vi.fn(),
  });
  mocks.useUpdateMonitor.mockReset().mockReturnValue({
    mutate: mocks.update,
    isPending: false,
    reset: vi.fn(),
  });
});

describe("MonitorFormDialog", () => {
  it("blocks an invalid URL before calling the API", async () => {
    renderDialog();
    await userEvent.type(screen.getByPlaceholderText("https://example.com/health"), "not-a-url");
    await userEvent.click(screen.getByRole("button", { name: "Create monitor" }));

    expect(
      await screen.findByText("Must be a valid URL (http:// or https://)"),
    ).toBeInTheDocument();
    expect(mocks.create).not.toHaveBeenCalled();
  });

  it("creates a monitor with the typed URL and default settings", async () => {
    const { onOpenChange } = renderDialog();
    await userEvent.type(
      screen.getByPlaceholderText("https://example.com/health"),
      "https://example.com",
    );
    await userEvent.click(screen.getByRole("button", { name: "Create monitor" }));

    await waitFor(() => {
      expect(mocks.create).toHaveBeenCalledWith(
        expect.objectContaining({
          url: "https://example.com",
          check_interval_seconds: 30,
        }),
        expect.objectContaining({ onSuccess: expect.any(Function) }),
      );
    });

    const options = mocks.create.mock.calls[0]?.[1] as { onSuccess: () => void };
    options.onSuccess();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("prefills and updates an existing monitor", async () => {
    renderDialog(editingMonitor());
    expect(await screen.findByDisplayValue("https://existing.example.com")).toBeInTheDocument();

    const name = screen.getByPlaceholderText("Optional — defaults to the hostname");
    await userEvent.clear(name);
    await userEvent.type(name, "Renamed");
    await userEvent.click(screen.getByRole("button", { name: "Save changes" }));

    await waitFor(() => {
      expect(mocks.update).toHaveBeenCalledWith(
        expect.objectContaining({ name: "Renamed", url: "https://existing.example.com" }),
        expect.objectContaining({ onSuccess: expect.any(Function) }),
      );
    });
    expect(mocks.create).not.toHaveBeenCalled();
  });
});
