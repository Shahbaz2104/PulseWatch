import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WindowPicker } from "@/components/window-picker";

describe("WindowPicker", () => {
  it("defaults to the 24h preset", () => {
    render(<WindowPicker value={{ window: "24h" }} onChange={() => {}} />);
    expect(screen.getByRole("tab", { name: "24h" })).toHaveAttribute("data-state", "active");
  });

  it("calls onChange when a preset is selected", async () => {
    const onChange = vi.fn();
    render(<WindowPicker value={{ window: "24h" }} onChange={onChange} />);
    await userEvent.click(screen.getByRole("tab", { name: "7d" }));
    expect(onChange).toHaveBeenCalledWith({ window: "7d" });
  });

  it("switches to custom and reveals date inputs", async () => {
    const onChange = vi.fn();
    render(<WindowPicker value={{ window: "7d" }} onChange={onChange} allowCustom />);
    await userEvent.click(screen.getByRole("tab", { name: "Custom" }));
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ window: "custom", from: expect.any(String) }),
    );
  });

  it("hides Custom when allowCustom is false", () => {
    render(<WindowPicker value={{ window: "24h" }} onChange={() => {}} allowCustom={false} />);
    expect(screen.queryByRole("tab", { name: "Custom" })).not.toBeInTheDocument();
  });
});
