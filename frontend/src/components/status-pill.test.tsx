import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusPill } from "@/components/status-pill";

describe("StatusPill", () => {
  it("renders Up with the success status attribute", () => {
    render(<StatusPill status="up" />);
    const pill = screen.getByText("Up");
    expect(pill.closest("[data-status]")?.getAttribute("data-status")).toBe("up");
  });

  it("renders Down", () => {
    render(<StatusPill status="down" />);
    expect(screen.getByText("Down")).toBeInTheDocument();
  });

  it("renders Unknown and Paused", () => {
    render(<StatusPill status="unknown" />);
    expect(screen.getByText("Unknown")).toBeInTheDocument();
  });

  it("shows Degraded when nominally up but degraded", () => {
    render(<StatusPill status="up" degraded />);
    const pill = screen.getByText("Degraded");
    expect(pill.closest("[data-status]")?.getAttribute("data-status")).toBe("degraded");
  });

  it("does not label paused as degraded", () => {
    render(<StatusPill status="paused" degraded />);
    expect(screen.getByText("Paused")).toBeInTheDocument();
  });
});
