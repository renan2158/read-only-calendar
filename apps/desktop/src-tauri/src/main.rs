#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use calendar_core::{Account, Settings};
use calendar_providers::{Cancellation, Context, Credentials};
use calendar_storage::Store;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use zeroize::Zeroizing;

type Shared = Arc<Mutex<App>>;
type Result<T> = std::result::Result<T, String>;
struct App {
    directory: PathBuf,
    store: Option<Store>,
    cancel: Cancellation,
    generation: u64,
    busy: bool,
    last_activity: Instant,
    last_tick: i64,
    last_poll: Instant,
    notice: Option<String>,
    monitor_available: bool,
    pending_event: Option<String>,
    external_flow: bool,
}
impl App {
    fn lock(&mut self) {
        self.cancel.cancel();
        self.store.take();
        self.generation += 1;
        self.busy = false;
        self.last_activity = Instant::now();
    }
    fn store(&self) -> Result<&Store> {
        self.store
            .as_ref()
            .ok_or_else(|| "Unlock your vault to continue.".into())
    }
    fn store_mut(&mut self) -> Result<&mut Store> {
        self.store
            .as_mut()
            .ok_or_else(|| "Unlock your vault to continue.".into())
    }
    fn unlocked(&mut self, store: Store) {
        self.lock();
        self.store = Some(store);
        self.cancel = Cancellation::default();
        self.last_tick = chrono::Utc::now().timestamp() - 3600;
        self.last_poll = Instant::now() - Duration::from_secs(86400);
    }
}
fn guarded(shared: &Shared) -> Result<std::sync::MutexGuard<'_, App>> {
    shared
        .lock()
        .map_err(|_| "The application could not access its vault state.".into())
}
fn entry(id: &str) -> Result<keyring::Entry> {
    keyring::Entry::new("org.renan.readonlycalendar", id)
        .map_err(|_| "OS credential storage is unavailable. No insecure fallback is used.".into())
}
fn read_credentials(store: &Store, id: &str) -> Result<Credentials> {
    let wrapped = Zeroizing::new(entry(id)?.get_password().map_err(|_| {
        "Reconnect this account: its OS credential entry is missing or locked.".to_owned()
    })?);
    let bytes = store.open_secret(id, &wrapped).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|_| "The credential entry could not be opened.".into())
}
fn save_credentials(store: &Store, id: &str, c: &Credentials) -> Result<()> {
    let bytes = Zeroizing::new(
        serde_json::to_vec(c).map_err(|_| "Could not protect account credentials.".to_owned())?,
    );
    let wrapped = Zeroizing::new(store.seal_secret(id, &bytes).map_err(|e| e.to_string())?);
    entry(id)?.set_password(&wrapped).map_err(|_|"Unlock your operating-system credential store and try again. Nothing was saved insecurely.".into())
}

#[tauri::command]
fn status(state: State<Shared>) -> Result<Value> {
    let a = guarded(&state)?;
    Ok(
        json!({"initialized":Store::exists(&a.directory),"locked":a.store.is_none(),"busy":a.busy,"notice":a.notice,"monitorAvailable":a.monitor_available}),
    )
}
#[tauri::command]
async fn unlock(
    state: State<'_, Shared>,
    passphrase: String,
    create: bool,
    timezone: String,
) -> Result<()> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let pass = Zeroizing::new(passphrase);
        let mut a = guarded(&shared)?;
        let mut store = if create {
            Store::create(&a.directory, pass.as_bytes())
        } else {
            Store::open(&a.directory, pass.as_bytes())
        }
        .map_err(|e| e.to_string())?;
        if create {
            let mut s = store.load().map_err(|e| e.to_string())?;
            s.settings.timezone = timezone;
            s.settings.validate().map_err(|e| e.to_string())?;
            store.save(&s).map_err(|e| e.to_string())?;
        }
        a.unlocked(store);
        Ok(())
    })
    .await
    .map_err(|_| "Could not unlock the vault.".to_owned())?
}
#[tauri::command]
fn lock(app: tauri::AppHandle, state: State<Shared>) -> Result<()> {
    guarded(&state)?.lock();
    let _ = app.notification().remove_all_active();
    let _ = app.emit("vault-locked", ());
    Ok(())
}
#[tauri::command]
fn activity(state: State<Shared>) -> Result<()> {
    guarded(&state)?.last_activity = Instant::now();
    Ok(())
}
#[tauri::command]
fn view(state: State<Shared>, from: i64, to: i64) -> Result<Value> {
    let mut a = guarded(&state)?;
    let snapshot = a.store()?.load().map_err(|e| e.to_string())?;
    let mut expanded =
        calendar_core::expand(&snapshot.events, from, to).map_err(|e| e.to_string())?;
    if let Some(id) = &a.pending_event {
        if !expanded.iter().any(|e| &e.id == id) {
            let now = chrono::Utc::now().timestamp();
            if let Ok(all) = calendar_core::expand(&snapshot.events, now - 86400, now + 8 * 86400) {
                if let Some(event) = all.into_iter().find(|e| &e.id == id) {
                    expanded.push(event);
                }
            }
        }
    }
    let events:Vec<Value>=expanded.iter().map(|e|json!({"id":e.id,"calendarId":e.calendar_id,"title":e.title,"start":e.start,"end":e.end,"allDay":e.all_day,"timezone":e.timezone,"description":e.description,"location":e.location,"organizer":e.organizer,"attendees":e.attendees})).collect();
    let calendars:Vec<Value>=snapshot.calendars.iter().map(|c|json!({"id":c.id,"accountId":c.account_id,"name":c.name,"color":c.color,"visible":c.visible,"lastSync":c.last_sync,"error":c.error})).collect();
    let pending = a.pending_event.take();
    Ok(
        json!({"accounts":snapshot.accounts,"calendars":calendars,"events":events,"settings":snapshot.settings,"busy":a.busy,"notice":a.notice,"pendingEvent":pending}),
    )
}
#[tauri::command]
fn save_settings(state: State<Shared>, settings: Settings) -> Result<()> {
    settings.validate().map_err(|e| e.to_string())?;
    let mut a = guarded(&state)?;
    let store = a.store_mut()?;
    let mut s = store.load().map_err(|e| e.to_string())?;
    s.settings = settings;
    store.save(&s).map_err(|e| e.to_string())
}
#[tauri::command]
fn calendar_preferences(
    state: State<Shared>,
    id: String,
    visible: bool,
    color: String,
) -> Result<()> {
    if color.len() != 7
        || !color.starts_with('#')
        || !color[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err("Choose a valid calendar color.".into());
    }
    let mut a = guarded(&state)?;
    let store = a.store_mut()?;
    let mut s = store.load().map_err(|e| e.to_string())?;
    let c = s
        .calendars
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or("Calendar not found.")?;
    c.visible = visible;
    c.color = color;
    store.save(&s).map_err(|e| e.to_string())
}
#[tauri::command]
fn add_account(state: State<Shared>, name: String, credentials: Credentials) -> Result<()> {
    if name.trim().is_empty() || name.len() > 120 {
        return Err("Enter an account name (up to 120 characters).".into());
    }
    match &credentials {
        Credentials::Ics { url }
        | Credentials::Proton { url }
        | Credentials::Caldav { url, .. } => {
            calendar_providers::ReadClient::endpoint(url).map_err(|e| e.to_string())?;
        }
        Credentials::Google { .. } => return Err("Use Connect Google to authorize access.".into()),
    }
    let mut a = guarded(&state)?;
    let store = a.store_mut()?;
    let mut s = store.load().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    save_credentials(store, &id, &credentials)?;
    s.accounts.push(Account {
        id,
        provider: credentials.provider().into(),
        name,
    });
    store.save(&s).map_err(|e| e.to_string())
}
#[tauri::command]
async fn connect_google(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    name: String,
) -> Result<()> {
    struct FlowGuard(Shared);
    impl Drop for FlowGuard {
        fn drop(&mut self) {
            if let Ok(mut a) = self.0.lock() {
                a.external_flow = false;
            }
        }
    }
    guarded(&state)?.external_flow = true;
    let _flow = FlowGuard(state.inner().clone());
    let path = app
        .dialog()
        .file()
        .add_filter("Desktop OAuth client", &["json"])
        .blocking_pick_file()
        .ok_or("No client configuration selected.")?
        .into_path()
        .map_err(|_| "Select a local Desktop OAuth client JSON file.")?;
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if std::fs::metadata(&path)
            .map_err(|_| "Cannot read client configuration.")?
            .len()
            > 65536
        {
            return Err("Client configuration is too large.".into());
        }
        let config =
            Zeroizing::new(std::fs::read(path).map_err(|_| "Cannot read client configuration.")?);
        let json: Value = serde_json::from_slice(&config)
            .map_err(|_| "Select a valid Google Desktop OAuth client JSON file.")?;
        let client = json
            .get("installed")
            .ok_or("A Desktop OAuth client is required.")?;
        let client_id = client["client_id"].as_str().ok_or("Missing client ID.")?;
        let secret = client["client_secret"].as_str().unwrap_or("");
        let (generation, cancel) = {
            let a = guarded(&shared)?;
            a.store()?;
            (a.generation, a.cancel.clone())
        };
        let credentials = calendar_providers::google::authorize(client_id, secret, cancel, |url| {
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|_| calendar_providers::Error::OAuth)
        })
        .map_err(|e| e.to_string())?;
        let mut a = guarded(&shared)?;
        if generation != a.generation {
            return Err("Authorization was cancelled because the vault locked.".into());
        }
        let store = a.store_mut()?;
        let id = uuid::Uuid::new_v4().to_string();
        save_credentials(store, &id, &credentials)?;
        let mut s = store.load().map_err(|e| e.to_string())?;
        s.accounts.push(Account {
            id,
            provider: "google".into(),
            name: if name.trim().is_empty() {
                "Google Calendar".into()
            } else {
                name
            },
        });
        store.save(&s).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Google authorization could not complete.".to_owned())?
}
#[tauri::command]
fn remove_account(state: State<Shared>, id: String) -> Result<()> {
    let mut a = guarded(&state)?;
    let store = a.store_mut()?;
    let mut s = store.load().map_err(|e| e.to_string())?;
    match entry(&id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(_) => {
            return Err("Unlock your OS credential store before removing this account.".into());
        }
    }
    s.accounts.retain(|x| x.id != id);
    let removed: Vec<_> = s
        .calendars
        .iter()
        .filter(|c| c.account_id == id)
        .map(|c| c.id.clone())
        .collect();
    s.calendars.retain(|c| c.account_id != id);
    s.events.retain(|e| !removed.contains(&e.calendar_id));
    store.save(&s).map_err(|e| e.to_string())
}
fn sync_all(shared: &Shared) -> Result<()> {
    let (accounts, generation, cancel) = {
        let mut a = guarded(shared)?;
        if a.busy {
            return Ok(());
        }
        let accounts = a.store()?.load().map_err(|e| e.to_string())?.accounts;
        a.busy = true;
        a.last_poll = Instant::now();
        (accounts, a.generation, a.cancel.clone())
    };
    for account in accounts {
        let (snapshot, credentials) = {
            let a = guarded(shared)?;
            if a.generation != generation {
                return Ok(());
            }
            let store = a.store()?;
            (
                store.load().map_err(|e| e.to_string())?,
                read_credentials(store, &account.id),
            )
        };
        let result = credentials.and_then(|credentials| {
            calendar_providers::sync(
                &Context {
                    account_id: &account.id,
                    calendars: &snapshot.calendars,
                    cached: &snapshot.events,
                    timezone: &snapshot.settings.timezone,
                    cancellation: cancel.clone(),
                },
                &credentials,
            )
            .map_err(|e| e.to_string())
        });
        let result = result.and_then(|result| {
            let now = chrono::Utc::now().timestamp();
            for c in &result.calendars {
                calendar_core::expand(&c.events, now - 31 * 86400, now + 335 * 86400)
                    .map_err(|e| e.to_string())?;
            }
            Ok(result)
        });
        let mut a = guarded(shared)?;
        if generation != a.generation {
            return Ok(());
        }
        let store = a.store_mut()?;
        let mut latest = store.load().map_err(|e| e.to_string())?;
        if !latest.accounts.iter().any(|x| x.id == account.id) {
            continue;
        }
        match result {
            Ok(result) => {
                let current_preferences = latest.calendars.clone();
                let removed: Vec<_> = latest
                    .calendars
                    .iter()
                    .filter(|c| c.account_id == account.id)
                    .map(|c| c.id.clone())
                    .collect();
                latest.events.retain(|e| !removed.contains(&e.calendar_id));
                latest.calendars.retain(|c| c.account_id != account.id);
                for mut item in result.calendars {
                    if let Some(old) = current_preferences
                        .iter()
                        .find(|c| c.id == item.calendar.id)
                    {
                        item.calendar.visible = old.visible;
                        item.calendar.color = old.color.clone();
                    }
                    latest.calendars.push(item.calendar);
                    latest.events.extend(item.events);
                }
            }
            Err(error) => {
                for c in &mut latest.calendars {
                    if c.account_id == account.id {
                        c.error = Some(error.clone());
                    }
                }
                if latest.calendars.iter().all(|c| c.account_id != account.id) {
                    latest.calendars.push(calendar_core::Calendar {
                        id: calendar_core::identity(&[&account.id, "pending"]),
                        account_id: account.id.clone(),
                        source_id: "pending".into(),
                        name: account.name.clone(),
                        color: "#668b80".into(),
                        visible: true,
                        last_sync: None,
                        error: Some(error),
                        cursor: None,
                    });
                }
            }
        }
        store.save(&latest).map_err(|e| e.to_string())?;
    }
    let mut a = guarded(shared)?;
    if a.generation == generation {
        a.busy = false;
    }
    Ok(())
}
#[tauri::command]
async fn sync_now(state: State<'_, Shared>) -> Result<()> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = sync_all(&shared);
        if result.is_err() {
            if let Ok(mut a) = guarded(&shared) {
                a.busy = false;
            }
        }
        result
    })
    .await
    .map_err(|_| "Synchronization could not complete.".to_owned())?
}
#[tauri::command]
async fn change_passphrase(state: State<'_, Shared>, current: String, next: String) -> Result<()> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let old = Zeroizing::new(current);
        let new = Zeroizing::new(next);
        guarded(&shared)?
            .store()?
            .change_passphrase(old.as_bytes(), new.as_bytes())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Could not change the passphrase.".to_owned())?
}
#[tauri::command]
async fn backup(app: tauri::AppHandle, state: State<'_, Shared>, passphrase: String) -> Result<()> {
    let path = app
        .dialog()
        .file()
        .set_file_name("calendar-backup.rcvault")
        .add_filter("Encrypted calendar backup", &["rcvault"])
        .blocking_save_file()
        .ok_or("Backup cancelled.")?
        .into_path()
        .map_err(|_| "Choose a local backup file.")?;
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let pass = Zeroizing::new(passphrase);
        guarded(&shared)?
            .store()?
            .backup(&path, pass.as_bytes())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Could not create the encrypted backup.".to_owned())?
}
#[tauri::command]
async fn restore(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    passphrase: String,
    new_passphrase: String,
) -> Result<()> {
    let path = app
        .dialog()
        .file()
        .add_filter("Encrypted calendar backup", &["rcvault"])
        .blocking_pick_file()
        .ok_or("Restore cancelled.")?
        .into_path()
        .map_err(|_| "Choose a local backup file.")?;
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let pass = Zeroizing::new(passphrase);
        let new = Zeroizing::new(new_passphrase);
        let mut a = guarded(&shared)?;
        let store = Store::restore(&a.directory, &path, pass.as_bytes(), new.as_bytes())
            .map_err(|e| e.to_string())?;
        a.unlocked(store);
        Ok(())
    })
    .await
    .map_err(|_| "Could not restore the encrypted backup.".to_owned())?
}
#[tauri::command]
fn device_unlock(state: State<Shared>, enabled: bool) -> Result<()> {
    let a = guarded(&state)?;
    let store = a.store()?;
    if enabled {
        entry("device-unlock")?
            .set_password(&hex::encode(store.device_key()))
            .map_err(|_| "OS credential storage is unavailable.".to_owned())?;
    } else {
        match entry("device-unlock")?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => return Err("Could not remove the device unlock key.".into()),
        }
    }
    Ok(())
}
#[tauri::command]
fn quit(app: tauri::AppHandle, state: State<Shared>) -> Result<()> {
    guarded(&state)?.lock();
    app.exit(0);
    Ok(())
}

fn os_locked() -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        for (dest, path) in [
            ("org.freedesktop.ScreenSaver", "/ScreenSaver"),
            ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"),
        ] {
            if let Ok(r) = std::process::Command::new("gdbus")
                .args([
                    "call",
                    "--session",
                    "--dest",
                    dest,
                    "--object-path",
                    path,
                    "--method",
                    &format!("{dest}.GetActive"),
                ])
                .output()
            {
                if r.status.success() {
                    let out = String::from_utf8_lossy(&r.stdout);
                    if out.contains("true") {
                        return Some(true);
                    }
                    if out.contains("false") {
                        return Some(false);
                    }
                }
            }
        }
        None
    }
    #[cfg(target_os = "windows")]
    {
        unsafe {
            use windows_sys::Win32::System::StationsAndDesktops::{
                CloseDesktop, DESKTOP_READOBJECTS, OpenInputDesktop,
            };
            let desk = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
            if desk.is_null() {
                Some(true)
            } else {
                CloseDesktop(desk);
                Some(false)
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("ioreg")
            .args(["-n", "Root", "-d1"])
            .output()
            .ok()
            .and_then(|r| {
                let text = String::from_utf8_lossy(&r.stdout);
                if text.contains("\"CGSSessionScreenIsLocked\" = Yes") {
                    Some(true)
                } else if r.status.success() {
                    Some(false)
                } else {
                    None
                }
            })
    }
}
fn tick(app: &tauri::AppHandle, shared: &Shared) -> Result<bool> {
    let mut a = guarded(shared)?;
    if a.store.is_none() {
        return Ok(false);
    }
    let snapshot = a.store()?.load().map_err(|e| e.to_string())?;
    if snapshot.settings.idle_minutes > 0
        && a.last_activity.elapsed()
            > Duration::from_secs(snapshot.settings.idle_minutes as u64 * 60)
    {
        a.lock();
        let _ = app.emit("vault-locked", ());
        let _ = app.notification().remove_all_active();
        return Ok(false);
    }
    let now = chrono::Utc::now().timestamp();
    let from = a.last_tick.max(now - snapshot.settings.missed_minutes * 60);
    a.last_tick = now;
    let events: Vec<_> = snapshot
        .events
        .iter()
        .filter(|e| {
            snapshot
                .calendars
                .iter()
                .any(|c| c.id == e.calendar_id && c.visible)
        })
        .cloned()
        .collect();
    let expanded =
        calendar_core::expand(&events, now - 86400, now + 8 * 86400).map_err(|e| e.to_string())?;
    let alerts = calendar_core::alerts(&expanded, &snapshot.settings, from, now)
        .map_err(|e| e.to_string())?;
    let pending: Vec<_> = alerts
        .into_iter()
        .filter(|x| !a.store().map(|s| s.delivered(&x.id)).unwrap_or(true))
        .collect();
    if !pending.is_empty() {
        let missed = pending.len() > 1 && pending.iter().any(|p| now - p.due > 30);
        if missed {
            let times = pending
                .iter()
                .rev()
                .take(3)
                .map(|p| p.body.split('·').next_back().unwrap_or("").trim())
                .collect::<Vec<_>>()
                .join(", ");
            let result = app
                .notification()
                .builder()
                .title(format!("{} missed calendar alerts", pending.len()))
                .body(format!("Recent event times: {times}"))
                .show();
            if result.is_ok() {
                for p in pending {
                    a.store()?.receipt(&p.id, now).map_err(|e| e.to_string())?;
                }
            } else {
                a.notice =
                    Some("Notifications could not be delivered. Check system permissions.".into());
            }
        } else {
            for p in pending {
                let result = app
                    .notification()
                    .builder()
                    .title(&p.title)
                    .body(&p.body)
                    .extra("eventId", json!(p.event_id))
                    .show();
                if result.is_ok() {
                    a.store()?.receipt(&p.id, now).map_err(|e| e.to_string())?;
                } else {
                    a.notice = Some(
                        "Notifications could not be delivered. Check system permissions.".into(),
                    );
                }
            }
        }
    }
    Ok(!a.busy
        && a.last_poll.elapsed() > Duration::from_secs(snapshot.settings.poll_minutes as u64 * 60))
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            status,
            unlock,
            lock,
            activity,
            view,
            save_settings,
            calendar_preferences,
            add_account,
            connect_google,
            remove_account,
            sync_now,
            change_passphrase,
            backup,
            restore,
            device_unlock,
            quit
        ])
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            let shared: Shared = Arc::new(Mutex::new(App {
                directory,
                store: None,
                cancel: Cancellation::default(),
                generation: 0,
                busy: false,
                last_activity: Instant::now(),
                last_tick: chrono::Utc::now().timestamp(),
                last_poll: Instant::now(),
                notice: None,
                monitor_available: false,
                pending_event: None,
                external_flow: false,
            }));
            // Auto-unlock is opt-in: this entry is created only by device_unlock(enabled=true).
            if let Ok(key) = entry("device-unlock").and_then(|e| {
                e.get_password()
                    .map_err(|_| "No device unlock entry.".into())
            }) {
                let key = Zeroizing::new(key);
                if let Ok(bytes) = hex::decode(key.as_str()) {
                    if bytes.len() == 32 {
                        let mut k = Zeroizing::new([0; 32]);
                        k.copy_from_slice(&bytes);
                        if let Ok(mut a) = shared.lock() {
                            if let Ok(s) = Store::open_key(&a.directory, k) {
                                a.unlocked(s);
                            }
                        }
                    }
                }
            }
            app.manage(shared.clone());
            let handle = app.handle().clone();
            app.notification().on_action(move |action| {
                let state = handle.state::<Shared>();
                if let Ok(mut a) = state.lock() {
                    a.pending_event = action
                        .notification()
                        .and_then(|n| n.extra().get("eventId"))
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);
                }
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
                let _ = handle.emit("notification-open", ());
            })?;
            let show =
                tauri::menu::MenuItem::with_id(app, "show", "Open calendar", true, None::<&str>)?;
            let lock =
                tauri::menu::MenuItem::with_id(app, "lock", "Lock vault", true, None::<&str>)?;
            let quit = tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&show, &lock, &quit])?;
            let mut tray = tauri::tray::TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Read-only Calendar")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "lock" => {
                        if let Ok(mut a) = app.state::<Shared>().lock() {
                            a.lock();
                        }
                        let _ = app.notification().remove_all_active();
                        let _ = app.emit("vault-locked", ());
                    }
                    "quit" => {
                        if let Ok(mut a) = app.state::<Shared>().lock() {
                            a.lock();
                        }
                        app.exit(0);
                    }
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(2));
                    let locked = os_locked();
                    if let Ok(mut a) = shared.lock() {
                        a.monitor_available = locked.is_some();
                        if locked == Some(true) && a.store.is_some() {
                            a.lock();
                            let _ = handle.emit("vault-locked", ());
                            let _ = handle.notification().remove_all_active();
                        }
                    }
                    match tick(&handle, &shared) {
                        Ok(true) => {
                            let state = shared.clone();
                            std::thread::spawn(move || {
                                let result = sync_all(&state);
                                if let Err(e) = result {
                                    if let Ok(mut a) = state.lock() {
                                        a.busy = false;
                                        a.notice = Some(e);
                                    }
                                }
                            });
                        }
                        Ok(false) => {}
                        Err(e) => {
                            if let Ok(mut a) = shared.lock() {
                                a.notice = Some(e);
                            }
                        }
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            let state = window.state::<Shared>();
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if let Ok(a) = state.lock() {
                        let background = a
                            .store
                            .as_ref()
                            .and_then(|s| s.load().ok())
                            .is_some_and(|s| s.settings.background);
                        if background {
                            api.prevent_close();
                            let _ = window.hide();
                        }
                    }
                }
                tauri::WindowEvent::Focused(false) => {
                    if let Ok(mut a) = state.lock() {
                        if !a.monitor_available && !a.external_flow {
                            a.lock();
                            let _ = window.emit("vault-locked", ());
                        }
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("Unable to initialize the desktop application");
}
