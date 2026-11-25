export interface ClockEventLike {
  event_type: string;
  occurred_at_epoch: string | number;
}

export function validateClockEvent(
  eventType: string,
  events: ClockEventLike[],
): string | null {
  const ordered = [...events].sort(
    (a, b) => Number(a.occurred_at_epoch) - Number(b.occurred_at_epoch),
  );

  let open = false;
  let breakOpen = false;
  for (const event of ordered) {
    switch (event.event_type) {
      case "clock_in":
        open = true;
        breakOpen = false;
        break;
      case "break_start":
        if (open) breakOpen = true;
        break;
      case "break_end":
        breakOpen = false;
        break;
      case "clock_out":
        open = false;
        breakOpen = false;
        break;
      default:
        break;
    }
  }

  switch (eventType) {
    case "clock_in":
      return open ? "Already clocked in" : null;
    case "clock_out":
      return open ? null : "No open clock-in to close";
    case "break_start":
      if (!open) return "No open clock-in for a break";
      return breakOpen ? "Break already open" : null;
    case "break_end":
      if (!open) return "No open clock-in for a break";
      return breakOpen ? null : "No open break to end";
    default:
      return "Invalid event type";
  }
}
