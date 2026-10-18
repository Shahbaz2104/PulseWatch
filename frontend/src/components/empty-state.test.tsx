import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Activity } from "lucide-react";
import { EmptyState } from "@/components/empty-state";

describe("EmptyState", () => {
  it("renders the teaching title and description", () => {
    render(
      <EmptyState
        icon={Activity}
        title="No monitors yet"
        description="Add your first URL."
      />,
    );
    expect(screen.getByText("No monitors yet")).toBeInTheDocument();
    expect(screen.getByText("Add your first URL.")).toBeInTheDocument();
  });

  it("fires the action callback", async () => {
    const onClick = vi.fn();
    render(
      <EmptyState
        icon={Activity}
        title="Nothing here"
        description="…"
        action={{ label: "Add monitor", onClick }}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Add monitor" }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("hides the action button when none is provided", () => {
    render(<EmptyState icon={Activity} title="Empty" description="…" />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
