import { describe, expect, it } from "vitest";

import { validateClockEvent } from "./clock.js";

describe("validateClockEvent", () => {
  it("allows clock-in only when no period is open", () => {
    expect(validateClockEvent("clock_in", [])).toBeNull();
    expect(
      validateClockEvent("clock_in", [
        { event_type: "clock_in", occurred_at_epoch: 1 },
      ]),
    ).toBe("Already clocked in");
  });

  it("allows clock-out only when a period is open", () => {
    const events = [
      { event_type: "clock_in", occurred_at_epoch: 1 },
      { event_type: "clock_out", occurred_at_epoch: 2 },
    ];
    expect(validateClockEvent("clock_out", events)).toBe("No open clock-in to close");
    expect(validateClockEvent("clock_out", events.slice(0, 1))).toBeNull();
  });

  it("tracks open breaks", () => {
    const events = [
      { event_type: "clock_in", occurred_at_epoch: 1 },
      { event_type: "break_start", occurred_at_epoch: 2 },
    ];
    expect(validateClockEvent("break_end", events)).toBeNull();
    expect(validateClockEvent("break_start", events)).toBe("Break already open");
    expect(
      validateClockEvent("break_end", [
        ...events,
        { event_type: "break_end", occurred_at_epoch: 3 },
      ]),
    ).toBe("No open break to end");
  });
});
