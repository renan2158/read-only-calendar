import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import {
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  Plus,
  Search,
  Settings as SettingsIcon,
  LockKeyhole,
  RefreshCw,
  ShieldCheck,
  ArrowUpRight,
  X,
  Check,
  CloudOff,
  Clock3,
  MapPin,
  Users,
  EyeOff,
  LogOut,
  Download,
  KeyRound,
  CalendarCheck,
  Sun,
  Menu,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import { call, message, native } from "./bridge";
import type { Event, Settings, Status, View, Mode } from "./types";
import { eventOnDay, monthDays, range, time, weekDays } from "./calendar";

function Dialog({
  title,
  subtitle,
  children,
  onClose,
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
    return () => ref.current?.close();
  }, []);
  return (
    <dialog
      ref={ref}
      onCancel={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
    >
      <div className="dialog-head">
        <div>
          <h2>{title}</h2>
          {subtitle && <p>{subtitle}</p>}
        </div>
        <button className="icon" aria-label="Close dialog" onClick={onClose}>
          <X size={19} />
        </button>
      </div>
      {children}
    </dialog>
  );
}
function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
    </label>
  );
}
function Vault({
  initialized,
  onOpen,
  onRestore,
}: {
  initialized: boolean;
  onOpen: (pass: string, create: boolean) => Promise<void>;
  onRestore: (backup: string, next: string) => Promise<void>;
}) {
  const [pass, setPass] = useState(""),
    [confirm, setConfirm] = useState(""),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [restore, setRestore] = useState(false),
    [backupPass, setBackupPass] = useState("");
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    if ((!initialized || restore) && pass !== confirm) {
      setError("The passphrases do not match.");
      return;
    }
    setBusy(true);
    try {
      if (restore) {
        await onRestore(backupPass, pass);
      } else await onOpen(pass, !initialized);
      setPass("");
      setConfirm("");
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="vault-page">
      <div className="wordmark">
        <span className="brand-mark">
          <CalendarDays size={21} />
        </span>
        read-only<span className="wordmark-light">calendar</span>
      </div>
      <main className="vault-card">
        <div className="vault-symbol">
          <LockKeyhole size={30} />
        </div>
        <div className="eyebrow">YOUR TIME. YOUR INFORMATION.</div>
        <h1>
          {restore
            ? "Restore your calendar"
            : initialized
              ? "A little privacy,\nbefore your day."
              : "Your calendars.\nA quieter home."}
        </h1>
        <p>
          {initialized
            ? "Unlock your encrypted vault to see your calendars."
            : "One calm place for your calendars. Encrypted on your device, connected directly to your providers."}
        </p>
        <form onSubmit={submit}>
          {restore && (
            <Field label="Backup passphrase">
              <input
                type="password"
                required
                autoComplete="off"
                value={backupPass}
                onChange={(e) => setBackupPass(e.target.value)}
              />
            </Field>
          )}
          <Field
            label={
              initialized && !restore ? "Passphrase" : "Create a passphrase"
            }
          >
            <input
              type="password"
              autoComplete={initialized ? "current-password" : "new-password"}
              autoFocus
              minLength={!initialized || restore ? 12 : 1}
              required
              value={pass}
              onChange={(e) => setPass(e.target.value)}
              placeholder={
                !initialized
                  ? "At least 12 characters"
                  : "Enter your passphrase"
              }
            />
          </Field>
          {(!initialized || restore) && (
            <Field label="Confirm passphrase">
              <input
                type="password"
                autoComplete="new-password"
                required
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
              />
            </Field>
          )}
          {error && (
            <div role="alert" className="error">
              {error}
            </div>
          )}
          <button className="primary full" disabled={busy || !native()}>
            {busy
              ? "Opening vault…"
              : restore
                ? "Choose encrypted backup"
                : initialized
                  ? "Unlock calendar"
                  : "Create encrypted vault"}
            <ArrowUpRight size={17} />
          </button>
        </form>
        {!initialized && (
          <>
            <p className="fine">
              Keep this passphrase somewhere safe. We cannot reset it or recover
              your local data.
            </p>
            <button
              className="text-button"
              onClick={() => {
                setRestore(!restore);
                setError("");
              }}
            >
              {restore
                ? "Create a new vault instead"
                : "Restore an encrypted backup"}
            </button>
          </>
        )}
        {!native() && (
          <div className="notice">
            Desktop application required. This browser view has no access to
            your data.
          </div>
        )}
        <div className="vault-foot">
          <ShieldCheck size={15} />
          Encrypted locally<span>·</span>No app account<span>·</span>Read only
        </div>
      </main>
      <div className="vault-bottom">More clarity. Less noise.</div>
    </div>
  );
}
export default function App() {
  const [status, setStatus] = useState<Status>({
    initialized: false,
    locked: true,
    busy: false,
    notice: null,
    monitorAvailable: true,
  });
  const [ready, setReady] = useState(false),
    [data, setData] = useState<View | null>(null);
  const [date, setDate] = useState(() => new Date()),
    [mode, setMode] = useState<Mode>("month"),
    [query, setQuery] = useState(""),
    [selected, setSelected] = useState<Event | null>(null),
    [modal, setModal] = useState<"connect" | "settings" | null>(null),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [sidebar, setSidebar] = useState(false);
  const epoch = useRef(0);
  const dataRef = useRef(data);
  dataRef.current = data;
  const statusRef = useRef(status);
  statusRef.current = status;
  const refresh = useCallback(async () => {
    const version = epoch.current;
    if (!native()) {
      setReady(true);
      return;
    }
    try {
      const s = await call<Status>("status");
      if (version !== epoch.current) return;
      setStatus(s);
      setReady(true);
      if (s.locked) {
        setData(null);
        setSelected(null);
        setModal(null);
        setQuery("");
        return;
      }
      const v = await call<View>("view", range(date));
      if (version !== epoch.current) return;
      setData(v);
      if (v.pendingEvent) {
        const event = v.events.find((e) => e.id === v.pendingEvent);
        if (event) setSelected(event);
      }
    } catch (e) {
      setError(message(e));
    }
  }, [date]);
  useEffect(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), 4000);
    return () => clearInterval(timer);
  }, [refresh]);
  useEffect(() => {
    if (!native()) return;
    let cleanup: (() => void)[] = [];
    let disposed = false;
    for (const ev of ["vault-locked", "notification-open"])
      listen(ev, () => {
        if (ev === "vault-locked") {
          epoch.current++;
          setStatus((s) => ({ ...s, locked: true }));
          setData(null);
          setSelected(null);
          setModal(null);
          setQuery("");
        }
        void refresh();
      }).then((unlisten) => {
        if (disposed) unlisten();
        else cleanup.push(unlisten);
      });
    return () => {
      disposed = true;
      cleanup.forEach((f) => f());
    };
  }, [refresh]);
  useEffect(() => {
    let last = 0;
    const active = () => {
      if (Date.now() - last > 30000 && !statusRef.current.locked) {
        last = Date.now();
        void call("activity").catch(() => {});
      }
    };
    window.addEventListener("pointerdown", active);
    window.addEventListener("keydown", active);
    return () => {
      window.removeEventListener("pointerdown", active);
      window.removeEventListener("keydown", active);
    };
  }, []);
  const operation = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError("");
    try {
      await action();
      await refresh();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  };
  const sync = () => operation(() => call("sync_now"));
  const unlocked = async (pass: string, create: boolean) => {
    await call("unlock", {
      passphrase: pass,
      create,
      timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    });
    await refresh();
    if (native() && !(await isPermissionGranted())) {
      await requestPermission();
    }
  };
  const lockNow = () =>
    operation(async () => {
      await call("lock");
      setData(null);
      setSelected(null);
      setModal(null);
    });
  if (!ready)
    return (
      <div className="loading">
        <ShieldCheck size={24} />
        <p>Opening your calendar…</p>
      </div>
    );
  if (status.locked || !data)
    return (
      <Vault
        initialized={status.initialized}
        onOpen={unlocked}
        onRestore={async (pass, next) => {
          await call("restore", { passphrase: pass, newPassphrase: next });
          await refresh();
        }}
      />
    );
  const zone = data.settings.timezone;
  const visible = data.events.filter((e) =>
    data.calendars.some((c) => c.id === e.calendarId && c.visible),
  );
  const filtered = visible.filter((e) =>
    `${e.title} ${e.description} ${e.location}`
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  const colors = new Map(data.calendars.map((c) => [c.id, c.color]));
  const today = new Date();
  const next = visible
    .filter((e) => e.end >= Date.now() / 1000)
    .sort((a, b) => a.start - b.start)
    .slice(0, 3);
  const change = (direction: number) => {
    const d = new Date(date);
    if (mode === "month") d.setMonth(d.getMonth() + direction, 1);
    else d.setDate(d.getDate() + direction * (mode === "week" ? 7 : 1));
    setDate(d);
  };
  const title = date.toLocaleDateString(undefined, {
    month: "long",
    year: "numeric",
  });
  const days =
    mode === "month"
      ? monthDays(date)
      : mode === "week"
        ? weekDays(date)
        : [date];
  const eventChip = (e: Event, compact = false) => (
    <button
      key={e.id}
      className={`event-chip ${compact ? "compact" : ""}`}
      style={
        {
          "--event-color": colors.get(e.calendarId) || "#668b80",
        } as React.CSSProperties
      }
      onClick={() => setSelected(e)}
      title={`${e.title || "Untitled event"} · ${time(e, zone)}`}
    >
      <span className="event-dot" />
      {!compact && <time>{time(e, zone)}</time>}
      <span>{e.title || "Untitled event"}</span>
    </button>
  );
  return (
    <div className="app-shell">
      <aside className={sidebar ? "sidebar open" : "sidebar"}>
        <div className="wordmark">
          <span className="brand-mark">
            <CalendarDays size={21} />
          </span>
          read-only
        </div>
        <div className="workspace-label">YOUR PERSONAL SPACE</div>
        <button className="nav-item active" onClick={() => setSidebar(false)}>
          <CalendarDays size={18} />
          Calendar<span className="nav-count">{data.calendars.length}</span>
        </button>
        <button className="connect-button" onClick={() => setModal("connect")}>
          <Plus size={16} />
          Connect a calendar
        </button>
        <div className="side-section">
          <div className="section-label">
            MY CALENDARS
            <button
              className="icon"
              aria-label="Connect calendar"
              onClick={() => setModal("connect")}
            >
              <Plus size={15} />
            </button>
          </div>
          {data.accounts.length === 0 ? (
            <p className="side-empty">
              Connect your first calendar to bring your day into focus.
            </p>
          ) : (
            data.accounts.map((account) => (
              <div className="account-group" key={account.id}>
                <div className="account-label">
                  {account.name}
                  <span>{account.provider}</span>
                </div>
                {data.calendars
                  .filter((c) => c.accountId === account.id)
                  .map((c) => (
                    <div className="calendar-row" key={c.id}>
                      <button
                        className="calendar-check"
                        style={{
                          background: c.visible ? c.color : "transparent",
                          borderColor: c.color,
                        }}
                        aria-label={`${c.visible ? "Hide" : "Show"} ${c.name}`}
                        aria-pressed={c.visible}
                        onClick={() =>
                          void operation(() =>
                            call("calendar_preferences", {
                              id: c.id,
                              visible: !c.visible,
                              color: c.color,
                            }),
                          )
                        }
                      >
                        {c.visible && <Check size={11} />}
                      </button>
                      <span title={c.error || c.name}>
                        {c.name}
                        {c.error && (
                          <CloudOff size={12} className="warning-icon" />
                        )}
                      </span>
                      <input
                        type="color"
                        aria-label={`Color for ${c.name}`}
                        value={c.color}
                        onChange={(e) =>
                          void operation(() =>
                            call("calendar_preferences", {
                              id: c.id,
                              visible: c.visible,
                              color: e.target.value,
                            }),
                          )
                        }
                      />
                    </div>
                  ))}
              </div>
            ))
          )}
        </div>
        <div className="side-bottom">
          <div className="privacy-card">
            <ShieldCheck size={19} />
            <div>
              <strong>Private by design</strong>
              <span>Encrypted on this device</span>
            </div>
            <span className="status-dot" />
          </div>
          <button className="nav-item" onClick={() => setModal("settings")}>
            <SettingsIcon size={17} />
            Settings
          </button>
          <button className="nav-item" onClick={() => void lockNow()}>
            <LockKeyhole size={17} />
            Lock vault
          </button>
          <div className="side-version">
            READ-ONLY CALENDAR <span>v0.1.0</span>
          </div>
        </div>
      </aside>
      <div className="main-shell">
        <header className="app-header">
          <div className="header-title">
            <button
              className="icon mobile-menu"
              aria-label="Toggle sidebar"
              onClick={() => setSidebar(!sidebar)}
            >
              <Menu size={20} />
            </button>
            <span>Calendar</span>
            <span className="pill">
              <LockKeyhole size={11} />
              Read only
            </span>
          </div>
          <div className="header-actions">
            <label className="search">
              <Search size={15} />
              <input
                aria-label="Search cached events"
                placeholder="Search your events"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
              <kbd>⌕</kbd>
            </label>
            <button
              className={`icon ${busy || data.busy ? "spinning" : ""}`}
              aria-label="Synchronize calendars"
              title="Synchronize calendars"
              onClick={() => void sync()}
              disabled={busy || data.busy}
            >
              <RefreshCw size={17} />
            </button>
            <button
              className="avatar"
              aria-label="Open settings"
              onClick={() => setModal("settings")}
            >
              R<span />
            </button>
          </div>
        </header>
        <main className="calendar-main">
          <div className="page-intro">
            <div>
              <div className="eyebrow">A LITTLE SPACE FOR YOUR DAY</div>
              <h1>
                Your time, in view<span>.</span>
              </h1>
              <p>Everything coming up. All in one calm place.</p>
            </div>
            <div className="today-summary">
              <Sun size={19} />
              <span>
                {today.toLocaleDateString(undefined, {
                  weekday: "long",
                  month: "short",
                  day: "numeric",
                })}
              </span>
            </div>
          </div>
          {(error || data.notice) && (
            <div role="alert" className="error dismissible">
              {error || data.notice}
              <button
                className="icon"
                onClick={() => setError("")}
                aria-label="Dismiss error"
              >
                <X size={14} />
              </button>
            </div>
          )}
          {!status.monitorAvailable && (
            <div className="notice">
              System lock detection is unavailable. The vault also locks when
              this window loses focus.
            </div>
          )}
          <div className="calendar-toolbar">
            <div className="month-nav">
              <h2>{title}</h2>
              <button
                className="icon"
                aria-label="Previous period"
                onClick={() => change(-1)}
              >
                <ChevronLeft size={17} />
              </button>
              <button
                className="icon"
                aria-label="Next period"
                onClick={() => change(1)}
              >
                <ChevronRight size={17} />
              </button>
              <button
                className="today-button"
                onClick={() => setDate(new Date())}
              >
                Today
              </button>
            </div>
            <div className="view-tabs" role="group" aria-label="Calendar view">
              {(["day", "week", "month", "agenda"] as Mode[]).map((m) => (
                <button
                  key={m}
                  aria-pressed={mode === m}
                  className={mode === m ? "selected" : ""}
                  onClick={() => setMode(m)}
                >
                  {m[0].toUpperCase() + m.slice(1)}
                </button>
              ))}
            </div>
          </div>
          <div className={`calendar-surface ${mode}`}>
            {mode === "agenda" ? (
              <div className="agenda-list">
                {filtered.length === 0 ? (
                  <div className="empty-state">
                    <CalendarCheck size={32} />
                    <h3>A little room to breathe.</h3>
                    <p>
                      {data.accounts.length
                        ? "No events in this month match your calendars and search."
                        : "Connect a calendar to see your upcoming events."}
                    </p>
                    {!data.accounts.length && (
                      <button
                        className="primary"
                        onClick={() => setModal("connect")}
                      >
                        Connect a calendar
                        <Plus size={16} />
                      </button>
                    )}
                  </div>
                ) : (
                  filtered.map((e) => (
                    <div className="agenda-row" key={e.id}>
                      <div className="agenda-date">
                        <strong>
                          {new Date(e.start * 1000).toLocaleDateString(
                            undefined,
                            { day: "numeric", timeZone: zone },
                          )}
                        </strong>
                        <span>
                          {new Date(e.start * 1000).toLocaleDateString(
                            undefined,
                            { month: "short", timeZone: zone },
                          )}
                        </span>
                      </div>
                      {eventChip(e)}
                      <span className="agenda-calendar">
                        {
                          data.calendars.find((c) => c.id === e.calendarId)
                            ?.name
                        }
                      </span>
                    </div>
                  ))
                )}
              </div>
            ) : (
              <>
                <div
                  className="weekday-row"
                  style={{
                    gridTemplateColumns: `repeat(${mode === "day" ? 1 : 7},1fr)`,
                  }}
                >
                  {(mode === "day"
                    ? [date.toLocaleDateString(undefined, { weekday: "long" })]
                    : ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"]
                  ).map((d) => (
                    <span key={d}>{d}</span>
                  ))}
                </div>
                <div
                  className="date-grid"
                  style={{
                    gridTemplateColumns: `repeat(${mode === "day" ? 1 : 7},1fr)`,
                  }}
                >
                  {days.map((d) => {
                    const events = filtered.filter((e) =>
                      eventOnDay(e, d, zone),
                    );
                    const isToday = d.toDateString() === today.toDateString();
                    return (
                      <section
                        key={d.toISOString()}
                        className={`date-cell ${d.getMonth() !== date.getMonth() ? "other-month" : ""} ${isToday ? "is-today" : ""}`}
                      >
                        <button
                          className="date-number"
                          aria-label={`View ${d.toDateString()}`}
                          onClick={() => {
                            setDate(d);
                            setMode("day");
                          }}
                        >
                          {mode === "day"
                            ? d.toLocaleDateString(undefined, {
                                weekday: "short",
                                day: "numeric",
                              })
                            : d.getDate()}
                        </button>
                        <div className="cell-events">
                          {events
                            .slice(0, mode === "month" ? 3 : 100)
                            .map((e) => eventChip(e, mode === "month"))}
                          {mode === "month" && events.length > 3 && (
                            <button
                              className="more-events"
                              onClick={() => {
                                setDate(d);
                                setMode("day");
                              }}
                            >
                              +{events.length - 3} more
                            </button>
                          )}
                        </div>
                      </section>
                    );
                  })}
                </div>
              </>
            )}
            {!data.accounts.length && mode === "month" && (
              <div className="connect-overlay">
                <CalendarCheck size={27} />
                <h3>Your day starts here.</h3>
                <p>Bring your calendars together, privately.</p>
                <button className="primary" onClick={() => setModal("connect")}>
                  Connect a calendar
                  <Plus size={16} />
                </button>
              </div>
            )}
          </div>
          <div className="calendar-footer">
            <span>
              <span className="status-dot" />
              Your calendar data stays encrypted on this device
            </span>
            <span>
              {data.busy
                ? "Synchronizing…"
                : data.calendars.some((c) => c.error)
                  ? "Some calendars need attention"
                  : "Direct provider connections"}
              <ShieldCheck size={13} />
            </span>
          </div>
          <section className="up-next">
            <div className="section-label">
              AHEAD OF YOU<span>YOUR NEXT EVENTS</span>
            </div>
            {next.length ? (
              <div className="upcoming-cards">
                {next.map((e) => (
                  <button
                    key={e.id}
                    className="upcoming-card"
                    onClick={() => setSelected(e)}
                  >
                    <div>
                      <span
                        className="event-dot"
                        style={{ background: colors.get(e.calendarId) }}
                      />
                      {new Date(e.start * 1000).toLocaleDateString(undefined, {
                        weekday: "short",
                        month: "short",
                        day: "numeric",
                        timeZone: zone,
                      })}
                      <ArrowUpRight size={15} />
                    </div>
                    <h3>{e.title || "Untitled event"}</h3>
                    <span>
                      {time(e, zone)}
                      {e.location && ` · ${e.location}`}
                    </span>
                  </button>
                ))}
              </div>
            ) : (
              <div className="quiet-note">
                <span>Nothing coming up just yet.</span> A clear calendar can be
                a good thing.
              </div>
            )}
          </section>
        </main>
      </div>
      {selected && (
        <Dialog
          title={selected.title || "Untitled event"}
          subtitle="Event details · read only"
          onClose={() => setSelected(null)}
        >
          <div className="event-details">
            <p>
              <Clock3 size={17} />
              {new Date(selected.start * 1000).toLocaleDateString(undefined, {
                dateStyle: "full",
                timeZone: zone,
              })}
              <br />
              {time(selected, zone)}
              {!selected.allDay &&
                ` – ${new Intl.DateTimeFormat(undefined, { timeZone: zone, hour: "2-digit", minute: "2-digit" }).format(new Date(selected.end * 1000))}`}
              <small>{zone}</small>
            </p>
            {selected.location && (
              <p>
                <MapPin size={17} />
                {selected.location}
              </p>
            )}
            {selected.organizer && (
              <p>
                <Users size={17} />
                {selected.organizer}
              </p>
            )}
            {selected.attendees.length > 0 && (
              <div>
                <h3>Attendees</h3>
                {selected.attendees.map((a, i) => (
                  <p key={i}>{a}</p>
                ))}
              </div>
            )}
            {selected.description && (
              <div className="description">
                <h3>Description</h3>
                <p>{selected.description}</p>
              </div>
            )}
            <div className="notice">
              Event information comes from your provider. Changes and invitation
              responses are managed there.
            </div>
          </div>
        </Dialog>
      )}
      {modal === "connect" && (
        <Connect
          busy={busy}
          onClose={() => setModal(null)}
          onConnect={async (name, provider, values) => {
            setBusy(true);
            setError("");
            try {
              if (provider === "google") await call("connect_google", { name });
              else
                await call("add_account", {
                  name,
                  credentials: { provider, ...values },
                });
              setModal(null);
              await sync();
            } catch (e) {
              throw e;
            } finally {
              setBusy(false);
            }
          }}
        />
      )}
      {modal === "settings" && (
        <SettingsPanel
          data={data}
          busy={busy}
          onClose={() => setModal(null)}
          onSave={async (settings) => {
            await call("save_settings", { settings });
            await refresh();
          }}
          onRemove={(id) => operation(() => call("remove_account", { id }))}
        />
      )}
    </div>
  );
}
function Connect({
  onClose,
  onConnect,
  busy,
}: {
  onClose: () => void;
  busy: boolean;
  onConnect: (
    name: string,
    provider: string,
    values: Record<string, string>,
  ) => Promise<void>;
}) {
  const [provider, setProvider] = useState("google"),
    [name, setName] = useState(""),
    [url, setUrl] = useState(""),
    [username, setUsername] = useState(""),
    [password, setPassword] = useState(""),
    [error, setError] = useState("");
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    try {
      await onConnect(
        name ||
          (
            {
              google: "Google Calendar",
              proton: "Proton Calendar",
              ics: "Subscribed calendar",
              caldav: "CalDAV",
            } as Record<string, string>
          )[provider],
        provider,
        { url, username, password },
      );
      setUrl("");
      setPassword("");
    } catch (e) {
      setError(message(e));
    }
  };
  return (
    <Dialog
      title="Bring your calendars together"
      subtitle="Direct connections. Read-only access. Always."
      onClose={onClose}
    >
      <form onSubmit={submit}>
        <div className="provider-options">
          {[
            ["google", "G", "Google"],
            ["caldav", "C", "CalDAV"],
            ["proton", "P", "Proton"],
            ["ics", "↗", "ICS feed"],
          ].map(([id, icon, label]) => (
            <button
              key={id}
              type="button"
              aria-pressed={id === provider}
              className={id === provider ? "provider selected" : "provider"}
              onClick={() => setProvider(id)}
            >
              <span>{icon}</span>
              {label}
            </button>
          ))}
        </div>
        <Field label="Account name">
          <input
            placeholder="e.g. Personal"
            value={name}
            maxLength={120}
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        {provider === "google" ? (
          <div className="notice">
            <strong>Authorize in your browser</strong>
            <p>
              Choose a Google Desktop OAuth client JSON file, then sign in with
              Google. Only calendar read permissions are requested. Configure
              the Calendar API and consent screen in your own Google Cloud
              project for this development version.
            </p>
          </div>
        ) : (
          <>
            <Field
              label={
                provider === "caldav"
                  ? "CalDAV discovery URL"
                  : "Private HTTPS subscription link"
              }
            >
              <input
                type="password"
                required
                autoComplete="off"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="https://…"
              />
            </Field>
            {provider === "caldav" && (
              <>
                <Field label="Username">
                  <input
                    required
                    autoComplete="off"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                  />
                </Field>
                <Field label="App password">
                  <input
                    type="password"
                    required
                    autoComplete="off"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                  />
                </Field>
              </>
            )}
            {provider === "proton" && (
              <div className="notice">
                In Proton Calendar web settings, create a{" "}
                <strong>full-view sharing link</strong>. A paid Proton plan is
                required to generate it. Anyone holding that link can access its
                shared information. Events remain read-only here.
              </div>
            )}
          </>
        )}
        {error && (
          <div role="alert" className="error">
            {error}
          </div>
        )}
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={onClose}>
            Cancel
          </button>
          <button className="primary" disabled={busy}>
            {busy
              ? "Connecting…"
              : provider === "google"
                ? "Choose file & authorize"
                : "Connect calendar"}
            <ArrowUpRight size={16} />
          </button>
        </div>
        <p className="fine">
          <ShieldCheck size={13} /> Secrets are encrypted before being stored in
          your operating-system credential store.
        </p>
      </form>
    </Dialog>
  );
}
function SettingsPanel({
  data,
  busy,
  onClose,
  onSave,
  onRemove,
}: {
  data: View;
  busy: boolean;
  onClose: () => void;
  onSave: (settings: Settings) => Promise<void>;
  onRemove: (id: string) => Promise<void>;
}) {
  const [s, setS] = useState(data.settings),
    [error, setError] = useState(""),
    [saved, setSaved] = useState(false),
    [action, setAction] = useState<"backup" | "password" | null>(null),
    [current, setCurrent] = useState(""),
    [next, setNext] = useState(""),
    [saving, setSaving] = useState(false);
  const run = async (fn: () => Promise<unknown>) => {
    setError("");
    setSaving(true);
    try {
      await fn();
      setSaved(true);
      setCurrent("");
      setNext("");
      setAction(null);
    } catch (e) {
      setError(message(e));
    } finally {
      setSaving(false);
    }
  };
  return (
    <Dialog
      title="Your calendar, your preferences"
      subtitle="Privacy, reminders, and connections"
      onClose={onClose}
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void run(() => onSave(s));
        }}
      >
        <div className="settings-section">
          <h3>
            <ShieldCheck size={17} />
            Privacy & security
          </h3>
          <label className="toggle-row">
            <div>
              <strong>Hide event details</strong>
              <span>Notifications show “Upcoming event” and the time.</span>
            </div>
            <input
              type="checkbox"
              checked={s.hideDetails}
              onChange={(e) => setS({ ...s, hideDetails: e.target.checked })}
            />
          </label>
          <label className="toggle-row">
            <div>
              <strong>Keep running in background</strong>
              <span>
                Closing the window keeps sync and alerts active until the vault
                locks.
              </span>
            </div>
            <input
              type="checkbox"
              checked={s.background}
              onChange={(e) => setS({ ...s, background: e.target.checked })}
            />
          </label>
          <div className="field-grid">
            <Field label="Lock after inactivity (minutes; 0 disables)">
              <input
                type="number"
                min={0}
                max={1440}
                value={s.idleMinutes}
                onChange={(e) =>
                  setS({ ...s, idleMinutes: Number(e.target.value) })
                }
              />
            </Field>
            <Field label="Display time zone">
              <input
                required
                value={s.timezone}
                placeholder="America/Sao_Paulo"
                onChange={(e) => setS({ ...s, timezone: e.target.value })}
              />
            </Field>
          </div>
          <div className="button-row">
            <button
              type="button"
              className="secondary"
              onClick={() => setAction("backup")}
            >
              <Download size={15} />
              Encrypted backup
            </button>
            <button
              type="button"
              className="secondary"
              onClick={() => setAction("password")}
            >
              <KeyRound size={15} />
              Change passphrase
            </button>
          </div>
          <div className="button-row">
            <button
              type="button"
              className="text-button"
              onClick={() => {
                if (
                  window.confirm(
                    "Allow automatic unlocking using this device’s OS credential store? This reduces protection from other processes in your logged-in OS session.",
                  )
                )
                  void run(() => call("device_unlock", { enabled: true }));
              }}
            >
              Enable device unlock
            </button>
            <button
              type="button"
              className="text-button"
              onClick={() =>
                void run(() => call("device_unlock", { enabled: false }))
              }
            >
              Disable device unlock
            </button>
          </div>
        </div>
        <div className="settings-section">
          <h3>
            <Clock3 size={17} />
            Reminders
          </h3>
          <div className="field-grid">
            <Field label="Fallback reminder (minutes before)">
              <input
                type="number"
                min={0}
                max={10080}
                value={s.fallbackMinutes}
                onChange={(e) =>
                  setS({ ...s, fallbackMinutes: Number(e.target.value) })
                }
              />
            </Field>
            <Field label="All-day notification hour (0–23)">
              <input
                type="number"
                min={0}
                max={23}
                value={s.allDayHour}
                onChange={(e) =>
                  setS({ ...s, allDayHour: Number(e.target.value) })
                }
              />
            </Field>
            <Field label="Sync interval (minutes)">
              <input
                type="number"
                min={5}
                max={1440}
                value={s.pollMinutes}
                onChange={(e) =>
                  setS({ ...s, pollMinutes: Number(e.target.value) })
                }
              />
            </Field>
            <Field label="Missed-alert window (minutes)">
              <input
                type="number"
                min={1}
                max={1440}
                value={s.missedMinutes}
                onChange={(e) =>
                  setS({ ...s, missedMinutes: Number(e.target.value) })
                }
              />
            </Field>
          </div>
          <p className="fine">
            Provider popup reminders are respected when available. Notifications
            pause while locked and may be delayed by sleep, system permissions,
            or stale provider data.
          </p>
        </div>
        {data.accounts.length > 0 && (
          <div className="settings-section">
            <h3>Connected accounts</h3>
            {data.accounts.map((a) => (
              <div key={a.id} className="toggle-row">
                <div>
                  <strong>{a.name}</strong>
                  <span>{a.provider} · read only</span>
                </div>
                <button
                  type="button"
                  className="text-button danger"
                  disabled={busy}
                  onClick={() => {
                    if (
                      window.confirm(
                        "Remove this account, its local cached events, and its stored credentials? Existing backups and OS notification history may retain older information.",
                      )
                    )
                      void onRemove(a.id);
                  }}
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
        )}
        {error && (
          <div role="alert" className="error">
            {error}
          </div>
        )}
        {saved && (
          <p role="status" className="success">
            <Check size={14} />
            Saved on this device.
          </p>
        )}
        <div className="dialog-actions">
          <button
            type="button"
            className="text-button"
            onClick={() => void call("quit")}
          >
            <LogOut size={15} />
            Quit application
          </button>
          <button className="primary" disabled={saving}>
            {saving ? "Saving…" : "Save preferences"}
            <Check size={15} />
          </button>
        </div>
      </form>
      {action && (
        <div className="security-action">
          <h3>
            {action === "backup"
              ? "Protect your backup"
              : "Change vault passphrase"}
          </h3>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void run(() =>
                action === "backup"
                  ? call("backup", { passphrase: next })
                  : call("change_passphrase", { current, next }),
              );
            }}
          >
            {action === "password" && (
              <Field label="Current passphrase">
                <input
                  required
                  type="password"
                  autoComplete="current-password"
                  value={current}
                  onChange={(e) => setCurrent(e.target.value)}
                />
              </Field>
            )}
            <Field
              label={
                action === "backup"
                  ? "Backup passphrase (at least 12 characters)"
                  : "New passphrase (at least 12 characters)"
              }
            >
              <input
                required
                minLength={12}
                type="password"
                autoComplete="new-password"
                value={next}
                onChange={(e) => setNext(e.target.value)}
              />
            </Field>
            <div className="button-row">
              <button
                type="button"
                className="secondary"
                onClick={() => {
                  setAction(null);
                  setCurrent("");
                  setNext("");
                }}
              >
                Cancel
              </button>
              <button className="primary" disabled={saving}>
                {action === "backup"
                  ? "Choose backup destination"
                  : "Change passphrase"}
              </button>
            </div>
          </form>
        </div>
      )}
    </Dialog>
  );
}
