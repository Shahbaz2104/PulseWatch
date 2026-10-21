import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { LoginPage } from "@/pages/LoginPage";
import { ApiError } from "@/lib/api";

const mocks = vi.hoisted(() => ({
  useAuthStatus: vi.fn(),
  useLogin: vi.fn(),
  mutateAsync: vi.fn(),
}));

vi.mock("@/hooks/use-api", () => ({
  useAuthStatus: mocks.useAuthStatus,
  useLogin: mocks.useLogin,
}));

vi.mock("@/components/ambient-canvas", () => ({
  AmbientCanvas: () => null,
}));

function renderLogin() {
  return render(
    <MemoryRouter initialEntries={["/login"]}>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="/" element={<p>Dashboard home</p>} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.useAuthStatus.mockReset().mockReturnValue({
    data: { authenticated: false, demo_mode: false },
    isLoading: false,
  });
  mocks.useLogin.mockReset().mockReturnValue({
    mutateAsync: mocks.mutateAsync,
    isPending: false,
  });
  mocks.mutateAsync.mockReset().mockResolvedValue({ authenticated: true, demo_mode: false });
});

describe("LoginPage", () => {
  it("signs in with the submitted password and returns to the dashboard", async () => {
    renderLogin();
    await userEvent.type(screen.getByLabelText("Password"), "correct horse");
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));

    expect(mocks.mutateAsync).toHaveBeenCalledWith("correct horse");
    expect(await screen.findByText("Dashboard home")).toBeInTheDocument();
  });

  it("shows the server's rejection instead of navigating", async () => {
    mocks.mutateAsync.mockRejectedValue(
      new ApiError(401, "unauthorized", "invalid password", []),
    );
    renderLogin();
    await userEvent.type(screen.getByLabelText("Password"), "wrong");
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("invalid password");
    expect(screen.queryByText("Dashboard home")).not.toBeInTheDocument();
  });

  it("explains read-only demo mode instead of asking for a password", () => {
    mocks.useAuthStatus.mockReturnValue({
      data: { authenticated: false, demo_mode: true },
      isLoading: false,
    });
    renderLogin();

    expect(screen.getByText("Read-only demo")).toBeInTheDocument();
    expect(screen.queryByLabelText("Password")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: /View dashboard/ })).toHaveAttribute("href", "/");
  });

  it("redirects an already signed-in admin to the dashboard", async () => {
    mocks.useAuthStatus.mockReturnValue({
      data: { authenticated: true, demo_mode: false },
      isLoading: false,
    });
    renderLogin();

    expect(await screen.findByText("Dashboard home")).toBeInTheDocument();
  });
});
