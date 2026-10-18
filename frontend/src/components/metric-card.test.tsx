import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { MetricCard } from "@/components/metric-card";

describe("MetricCard", () => {
  it("renders an em dash for null values — never a fabricated number", () => {
    render(<MetricCard label="Uptime" value={null} format="percent" />);
    expect(screen.getByText("Uptime")).toBeInTheDocument();
    expect(screen.getByText("—")).toBeInTheDocument();
  });

  it("renders a formatted percent for numeric values", () => {
    render(<MetricCard label="Uptime" value={99.94} format="percent" />);
    expect(screen.getByLabelText("99.94 percent")).toBeInTheDocument();
  });

  it("renders ms for response times", () => {
    render(<MetricCard label="Response" value={142} format="ms" />);
    expect(screen.getByLabelText("142 milliseconds")).toBeInTheDocument();
  });

  it("shows a skeleton while loading", () => {
    const { container } = render(<MetricCard label="Uptime" value={99} format="percent" loading />);
    expect(container.querySelector(".animate-pulse")).toBeInTheDocument();
  });

  it("renders pre-formatted string values verbatim", () => {
    render(<MetricCard label="Downtime" value="1h 30m" />);
    expect(screen.getByText("1h 30m")).toBeInTheDocument();
  });
});
