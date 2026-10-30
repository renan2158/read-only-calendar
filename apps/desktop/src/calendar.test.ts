import { describe, it, expect } from "vitest";
import { eventOnDay, monthDays, time } from "./calendar";
import type { Event } from "./types";
const event = (start: number, end: number, allDay = false): Event => ({
  id: "synthetic",
  calendarId: "c",
  title: "Synthetic",
  start,
  end,
  allDay,
  timezone: "America/New_York",
  description: "",
  location: "",
  organizer: "",
  attendees: [],
});
describe("calendar dates", () => {
  it("keeps an exclusive all-day end on its original date", () => {
    const e = event(
      Date.parse("2026-11-01T04:00:00Z") / 1000,
      Date.parse("2026-11-02T05:00:00Z") / 1000,
      true,
    );
    expect(eventOnDay(e, new Date(2026, 10, 1, 12), "Asia/Tokyo")).toBe(true);
    expect(eventOnDay(e, new Date(2026, 10, 2, 12), "Asia/Tokyo")).toBe(false);
  });
  it("uses display time zone for timed events", () => {
    const e = event(
      Date.parse("2026-11-02T01:00:00Z") / 1000,
      Date.parse("2026-11-02T02:00:00Z") / 1000,
    );
    expect(eventOnDay(e, new Date(2026, 10, 1, 12), "America/New_York")).toBe(
      true,
    );
    expect(eventOnDay(e, new Date(2026, 10, 2, 12), "America/New_York")).toBe(
      false,
    );
  });
  it("does not put an event ending at midnight on the next day", () => {
    const e = event(
      Date.parse("2026-11-01T22:00:00Z") / 1000,
      Date.parse("2026-11-02T00:00:00Z") / 1000,
    );
    expect(eventOnDay(e, new Date(2026, 10, 2, 12), "UTC")).toBe(false);
  });
  it("renders a complete six-week month with Monday first", () => {
    const days = monthDays(new Date(2026, 10, 1));
    expect(days).toHaveLength(42);
    expect(days[0].getDay()).toBe(1);
    expect(days[41].getDay()).toBe(0);
  });
  it("labels all-day dates without inventing an event time", () =>
    expect(time(event(0, 86400, true), "UTC")).toBe("All day"));
});
