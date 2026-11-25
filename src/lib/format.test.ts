import { describe, expect, it } from "vitest";
import { formatDuration, formatHours, localIso } from "@/lib/format";

describe("formatDuration", () => {
  it("formats minutes and hours", () => {
    expect(formatDuration(0)).toBe("0m");
    expect(formatDuration(45)).toBe("45m");
    expect(formatDuration(120)).toBe("2h");
    expect(formatDuration(155)).toBe("2h 35m");
  });

  it("clamps negative values", () => {
    expect(formatDuration(-10)).toBe("0m");
  });
});

describe("formatHours", () => {
  it("converts minutes to decimal hours", () => {
    expect(formatHours(90)).toBe("1.50");
  });
});

describe("localIso", () => {
  it("includes the local UTC offset", () => {
    const value = localIso(new Date("2026-11-25T01:02:03Z"));
    expect(value).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2}$/);
  });
});
