import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
} from "@testing-library/react";
import "@testing-library/jest-dom/vitest";
const mocks = vi.hoisted(() => ({
  locked: false,
  listeners: new Map<string, () => void>(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((name: string, handler: () => void) => {
    mocks.listeners.set(name, handler);
    return Promise.resolve(() => mocks.listeners.delete(name));
  }),
}));
vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: () => Promise.resolve(true),
  requestPermission: () => Promise.resolve("granted"),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === "status")
      return {
        initialized: true,
        locked: mocks.locked,
        busy: false,
        notice: null,
        monitorAvailable: true,
      };
    if (command === "view")
      return {
        accounts: [{ id: "a", name: "Personal", provider: "ics" }],
        calendars: [
          {
            id: "c",
            accountId: "a",
            name: "My calendar",
            color: "#668b80",
            visible: true,
            lastSync: 1,
            error: null,
          },
        ],
        events: [
          {
            id: "e",
            calendarId: "c",
            title: "Synthetic confidential meeting",
            start: Math.floor(Date.now() / 1000),
            end: Math.floor(Date.now() / 1000) + 3600,
            allDay: false,
            timezone: "UTC",
            description: '<img src="https://tracking.invalid/pixel">',
            location: "",
            organizer: "",
            attendees: [],
          },
        ],
        settings: {
          hideDetails: false,
          timezone: "UTC",
          fallbackMinutes: 10,
          allDayHour: 9,
          background: false,
          idleMinutes: 15,
          pollMinutes: 15,
          missedMinutes: 60,
        },
        busy: false,
        notice: null,
        pendingEvent: null,
      };
    if (command === "lock") mocks.locked = true;
    return null;
  }),
}));
import App from "./App";
beforeEach(() => {
  cleanup();
  mocks.locked = false;
  mocks.listeners.clear();
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    value: {},
    configurable: true,
  });
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute("open");
  };
});
describe("private application interface", () => {
  it("never renders cached event titles while locked", async () => {
    mocks.locked = true;
    render(<App />);
    await screen.findByRole("button", { name: /Unlock calendar/ });
    expect(
      screen.queryByText("Synthetic confidential meeting"),
    ).not.toBeInTheDocument();
  });
  it("clears event details on a vault lock event", async () => {
    render(<App />);
    await screen.findByText("Your time, in view");
    expect(
      screen.getAllByText("Synthetic confidential meeting").length,
    ).toBeGreaterThan(0);
    mocks.locked = true;
    mocks.listeners.get("vault-locked")?.();
    await screen.findByRole("button", { name: /Unlock calendar/ });
    expect(
      screen.queryByText("Synthetic confidential meeting"),
    ).not.toBeInTheDocument();
  });
  it("renders event descriptions as text without loading external content", async () => {
    render(<App />);
    await screen.findByText("Your time, in view");
    fireEvent.click(screen.getAllByText("Synthetic confidential meeting")[0]);
    await screen.findByText('<img src="https://tracking.invalid/pixel">');
    expect(document.querySelector("img")).toBeNull();
  });
  it("explains that hiding event details preserves the notification time", async () => {
    render(<App />);
    await screen.findByText("Your time, in view");
    fireEvent.click(screen.getAllByRole("button", { name: "Settings" })[0]);
    await waitFor(() =>
      expect(
        screen.getByText("Notifications show “Upcoming event” and the time."),
      ).toBeInTheDocument(),
    );
  });
});
