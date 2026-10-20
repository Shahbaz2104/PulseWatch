import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SettingsPage } from "@/pages/SettingsPage";
import type { AppSettings } from "@/types/api";

const mocks = vi.hoisted(() => ({
  useSettings: vi.fn(),
  useUpdateSettings: vi.fn(),
  setTheme: vi.fn(),
}));

vi.mock("@/hooks/use-api", () => ({
  useSettings: mocks.useSettings,
  useUpdateSettings: mocks.useUpdateSettings,
}));

vi.mock("next-themes", () => ({
  useTheme: () => ({ theme: "dark", setTheme: mocks.setTheme }),
}));

const SETTINGS: AppSettings = {
  default_check_interval_seconds: 60,
  default_timeout_ms: 5000,
  default_retry_max_attempts: 2,
  default_retry_backoff_ms: 500,
  default_retry_backoff_max_ms: 10_000,
  default_expected_status_min: 200,
  default_expected_status_max: 399,
  max_concurrent_checks: 16,
};

beforeEach(() => {
  mocks.setTheme.mockReset();
  mocks.useUpdateSettings.mockReset().mockReturnValue({ mutate: vi.fn(), isPending: false });
  mocks.useSettings.mockReset().mockReturnValue({
    data: SETTINGS,
    isLoading: false,
    isError: false,
    refetch: vi.fn(),
  });
});

describe("SettingsPage", () => {
  it("seeds the form from the API and explains the restart requirement", () => {
    render(<SettingsPage />);
    expect(screen.getByText("Restart to apply concurrency changes")).toBeInTheDocument();
    expect(screen.getByLabelText("Check interval (seconds)")).toHaveValue(60);
    expect(screen.getByLabelText("Initial retry backoff (ms)")).toHaveValue(500);
    expect(screen.getByLabelText("Max retry backoff (ms)")).toHaveValue(10000);
    expect(screen.getByLabelText("Max concurrent checks")).toHaveValue(16);
  });

  it("switches the theme preference", async () => {
    render(<SettingsPage />);
    await userEvent.click(screen.getByRole("button", { name: "Light" }));
    expect(mocks.setTheme).toHaveBeenCalledWith("light");
    expect(screen.getByRole("button", { name: "Dark" })).toHaveAttribute("aria-pressed", "true");
  });

  it("shows a retryable error state", async () => {
    const refetch = vi.fn();
    mocks.useSettings.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      refetch,
    });
    render(<SettingsPage />);
    expect(screen.getByText("Couldn't load settings")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(refetch).toHaveBeenCalledTimes(1);
  });
});
