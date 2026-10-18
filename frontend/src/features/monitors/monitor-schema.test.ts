import { describe, expect, it } from "vitest";
import { monitorDefaults, monitorSchema } from "@/features/monitors/monitor-schema";

describe("monitorSchema", () => {
  it("accepts the shipped defaults shape after filling a URL", () => {
    const result = monitorSchema.safeParse({
      ...monitorDefaults,
      name: "Example",
      url: "https://example.com",
    });
    expect(result.success).toBe(true);
  });

  it("rejects a missing URL", () => {
    const result = monitorSchema.safeParse({ ...monitorDefaults, url: "" });
    expect(result.success).toBe(false);
  });

  it("rejects an invalid URL", () => {
    const result = monitorSchema.safeParse({ ...monitorDefaults, url: "not-a-url" });
    expect(result.success).toBe(false);
  });

  it("rejects an interval below 10 seconds", () => {
    const result = monitorSchema.safeParse({
      ...monitorDefaults,
      url: "https://example.com",
      check_interval_seconds: 5,
    });
    expect(result.success).toBe(false);
  });

  it("rejects expected_status_min greater than max", () => {
    const result = monitorSchema.safeParse({
      ...monitorDefaults,
      url: "https://example.com",
      expected_status_min: 400,
      expected_status_max: 200,
    });
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues.some((i) => i.path[0] === "expected_status_max")).toBe(true);
    }
  });

  it("coerces numeric strings from inputs", () => {
    const result = monitorSchema.safeParse({
      ...monitorDefaults,
      url: "https://example.com",
      check_interval_seconds: "30",
      timeout_ms: "5000",
    });
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.check_interval_seconds).toBe(30);
      expect(result.data.timeout_ms).toBe(5000);
    }
  });
});
