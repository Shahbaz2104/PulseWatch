import { describe, expect, it } from "vitest";
import {
  formatClock,
  formatHttpStatus,
  formatMs,
  formatPercent,
  formatRelative,
  formatSeconds,
} from "@/lib/format";

describe("formatPercent", () => {
  it("formats to one decimal by default", () => {
    expect(formatPercent(99.94)).toBe("99.9%");
    expect(formatPercent(100)).toBe("100.0%");
  });

  it("returns an em dash for null/undefined — never a fake 100%", () => {
    expect(formatPercent(null)).toBe("—");
    expect(formatPercent(undefined)).toBe("—");
    expect(formatPercent(Number.NaN)).toBe("—");
  });

  it("respects custom digits", () => {
    expect(formatPercent(99.999, 2)).toBe("100.00%");
  });
});

describe("formatMs", () => {
  it("shows milliseconds under one second", () => {
    expect(formatMs(142)).toBe("142 ms");
    expect(formatMs(0)).toBe("0 ms");
  });

  it("switches to seconds at 1000ms", () => {
    expect(formatMs(1500)).toBe("1.50 s");
  });

  it("returns an em dash for null", () => {
    expect(formatMs(null)).toBe("—");
  });
});

describe("formatSeconds", () => {
  it("formats short, minute, hour, day durations", () => {
    expect(formatSeconds(45)).toBe("45s");
    expect(formatSeconds(90)).toBe("1m 30s");
    expect(formatSeconds(120)).toBe("2m");
    expect(formatSeconds(3600)).toBe("1h");
    expect(formatSeconds(5400)).toBe("1h 30m");
    expect(formatSeconds(86_400)).toBe("1d 0h");
  });

  it("returns an em dash for null", () => {
    expect(formatSeconds(null)).toBe("—");
  });
});

describe("formatRelative", () => {
  it("returns an em dash for missing or invalid input", () => {
    expect(formatRelative(null)).toBe("—");
    expect(formatRelative("not-a-date")).toBe("—");
  });

  it("describes a past instant", () => {
    const past = new Date(Date.now() - 5 * 60_000).toISOString();
    expect(formatRelative(past)).toMatch(/ago$/);
  });
});

describe("formatClock", () => {
  it("returns an em dash for empty input", () => {
    expect(formatClock(undefined)).toBe("—");
  });

  it("formats a real ISO timestamp to a locale string", () => {
    const out = formatClock("2026-01-15T12:00:00Z");
    expect(out).not.toBe("—");
    expect(out.length).toBeGreaterThan(5);
  });
});

describe("formatHttpStatus", () => {
  it("stringifies codes and keeps null as em dash", () => {
    expect(formatHttpStatus(200)).toBe("200");
    expect(formatHttpStatus(null)).toBe("—");
  });
});
