import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { AuthAction } from "@/features/auth/auth-action";

const mocks = vi.hoisted(() => ({
  useAuthStatus: vi.fn(),
  useLogout: vi.fn(),
  mutate: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock("@/hooks/use-api", () => ({
  useAuthStatus: mocks.useAuthStatus,
  useLogout: mocks.useLogout,
}));

vi.mock("sonner", () => ({
  toast: { success: mocks.toastSuccess, error: mocks.toastError },
}));

function renderAction() {
  return render(
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route
          path="/"
          element={
            <>
              <AuthAction />
              <p>Dashboard home</p>
            </>
          }
        />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mocks.useAuthStatus.mockReset().mockReturnValue({
    data: { authenticated: true, demo_mode: false },
    isLoading: false,
  });
  mocks.mutate.mockReset();
  mocks.useLogout.mockReset().mockReturnValue({ mutate: mocks.mutate, isPending: false });
  mocks.toastSuccess.mockReset();
  mocks.toastError.mockReset();
});

describe("AuthAction", () => {
  it("links anonymous visitors to the login page", () => {
    mocks.useAuthStatus.mockReturnValue({
      data: { authenticated: false, demo_mode: false },
      isLoading: false,
    });
    renderAction();
    expect(screen.getByRole("link", { name: /Sign in/ })).toHaveAttribute("href", "/login");
  });

  it("marks demo deployments as read-only", () => {
    mocks.useAuthStatus.mockReturnValue({
      data: { authenticated: false, demo_mode: true },
      isLoading: false,
    });
    renderAction();
    expect(screen.getByText("Read-only demo")).toBeInTheDocument();
    expect(screen.queryByText("Sign in")).not.toBeInTheDocument();
  });

  it("signs the admin out and returns to the dashboard", async () => {
    mocks.mutate.mockImplementation((_vars, options) => options?.onSuccess?.());
    renderAction();
    await userEvent.click(screen.getByRole("button", { name: /Sign out/ }));

    expect(mocks.mutate).toHaveBeenCalledTimes(1);
    expect(mocks.toastSuccess).toHaveBeenCalledWith("Signed out");
    expect(screen.getByText("Dashboard home")).toBeInTheDocument();
  });
});
