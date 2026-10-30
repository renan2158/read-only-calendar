export interface Event {
  id: string;
  calendarId: string;
  title: string;
  start: number;
  end: number;
  allDay: boolean;
  timezone: string;
  description: string;
  location: string;
  organizer: string;
  attendees: string[];
}
export interface Calendar {
  id: string;
  accountId: string;
  name: string;
  color: string;
  visible: boolean;
  lastSync: number | null;
  error: string | null;
}
export interface Account {
  id: string;
  provider: "google" | "caldav" | "ics" | "proton";
  name: string;
}
export interface Settings {
  hideDetails: boolean;
  timezone: string;
  fallbackMinutes: number;
  allDayHour: number;
  background: boolean;
  idleMinutes: number;
  pollMinutes: number;
  missedMinutes: number;
}
export interface View {
  accounts: Account[];
  calendars: Calendar[];
  events: Event[];
  settings: Settings;
  busy: boolean;
  notice: string | null;
  pendingEvent: string | null;
}
export interface Status {
  initialized: boolean;
  locked: boolean;
  busy: boolean;
  notice: string | null;
  monitorAvailable: boolean;
}
export type Mode = "day" | "week" | "month" | "agenda";
