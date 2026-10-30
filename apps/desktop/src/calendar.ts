import type { Event } from "./types";
export function dayKey(date: Date, zone: string) {
  const p = new Intl.DateTimeFormat("en-CA", {
    timeZone: zone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(date);
  return `${p.find((x) => x.type === "year")?.value}-${p.find((x) => x.type === "month")?.value}-${p.find((x) => x.type === "day")?.value}`;
}
export function eventOnDay(event: Event, date: Date, zone: string) {
  const target = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
  const start = dayKey(
    new Date(event.start * 1000),
    event.allDay ? event.timezone : zone,
  );
  const end = dayKey(
    new Date(Math.max(event.start, event.end - 1) * 1000),
    event.allDay ? event.timezone : zone,
  );
  return target >= start && target <= end;
}
export function monthDays(date: Date) {
  const first = new Date(date.getFullYear(), date.getMonth(), 1, 12);
  const offset = (first.getDay() + 6) % 7;
  const start = new Date(first);
  start.setDate(1 - offset);
  return Array.from({ length: 42 }, (_, i) => {
    const d = new Date(start);
    d.setDate(d.getDate() + i);
    return d;
  });
}
export function time(event: Event, zone: string) {
  return event.allDay
    ? "All day"
    : new Intl.DateTimeFormat(undefined, {
        timeZone: zone,
        hour: "2-digit",
        minute: "2-digit",
      }).format(new Date(event.start * 1000));
}
export function range(date: Date) {
  return {
    from: Math.floor(
      new Date(date.getFullYear(), date.getMonth(), -6).getTime() / 1000,
    ),
    to: Math.floor(
      new Date(date.getFullYear(), date.getMonth() + 1, 8).getTime() / 1000,
    ),
  };
}
export function weekDays(date: Date) {
  const start = new Date(date);
  start.setHours(12, 0, 0, 0);
  start.setDate(start.getDate() - ((start.getDay() + 6) % 7));
  return Array.from({ length: 7 }, (_, i) => {
    const d = new Date(start);
    d.setDate(d.getDate() + i);
    return d;
  });
}
