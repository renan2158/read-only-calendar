# Read-only Calendar

A quiet, privacy-first home for your calendars. A desktop app for Linux, macOS, and Windows built with **Tauri 2, Rust, React/TypeScript, and SQLCipher-encrypted SQLite**.

**Version 0.1.0 is a development build.** Read the implementation status and outstanding live-account/platform verification in [docs/IMPLEMENTATION.md](docs/IMPLEMENTATION.md) before using it with important calendars. The application never writes calendar events.

## Features

- Google Calendar via desktop OAuth, read-only scopes, paginated initial/incremental synchronization, cancelled recurring instances, and expired-cursor recovery.
- Generic HTTPS CalDAV with principal/calendar-home discovery, ETag inventory, calendar-multiget, collection sync tokens, and fallback after an invalid token. Basic/app-password authentication initially.
- HTTPS ICS subscriptions with HTTP validators. Proton uses its **official shared full-view calendar link**, without private APIs or Proton credentials.
- Day, week, month, and agenda views; local cached search, multiple accounts, calendar visibility and colors, and read-only event details.
- Event-start alerts, provider popup reminders, configurable fallback reminders, all-day alerts, deduplication receipts, and missed-alert summaries.
- Notifications show the title and time by default. **Hide event details** conceals the title while keeping the time visible.
- Encrypted offline cache, passphrase unlock, optional device unlock, idle/manual/system-session locking, encrypted backup/restore, and passphrase change.
- Optional tray/background operation. Locking the vault pauses synchronization and alerts. Notifications remain best effort when the computer sleeps, permissions are denied, or feeds are stale.
- Direct provider connections. No app account, telemetry, central event server, or remote images in event descriptions.

## Run locally

Install Node.js 24 LTS, a current stable Rust toolchain (Rust 1.90+), and [Tauri's platform prerequisites](https://v2.tauri.app/start/prerequisites/). On Linux, install the WebKitGTK 4.1/GTK development libraries, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`, `libdbus-1-dev`, a C/C++ compiler, `make`, `perl`, and `pkg-config`. On Windows use Visual Studio C++ build tools and WebView2; on macOS use Xcode command-line tools. Bundled SQLCipher/OpenSSL compilation requires the native build tools on each target.

```sh
npm ci
npm run desktop -- dev
```

Create a vault with a passphrase of at least 12 characters. Store it safely: there is no password-reset server. Provider credentials and subscription links are encrypted with a vault-derived key before being saved to the OS credential store; a missing/locked credential store causes an error, never a plaintext fallback.

For a production frontend build and development desktop binary:

```sh
npm run build
npm run desktop -- build --no-bundle
```

`npm run dev` alone opens the frontend development server; it cannot access real vault data without Tauri. No demo data or fake success state is used by the application.

## Connect calendars

**Google:** create a Google Cloud project, enable the Calendar API, configure the consent screen/test user, and download a **Desktop app** OAuth client JSON. Choose Google in Connect calendar and select that local file. Sign in through the system browser. Only calendar-list and event read scopes are requested. The distributed app currently uses this bring-your-own-client development flow; a maintainer-owned, verified OAuth client is a release prerequisite. Do not commit client files or tokens.

**CalDAV:** enter the HTTPS discovery endpoint and username/app password. Redirects and cross-origin discovery are rejected to prevent credential forwarding; use the final same-origin discovery URL. OAuth-only CalDAV services need a dedicated authentication extension. Plain HTTP and invalid/self-signed certificates are not accepted by the production reader.

**Proton:** generate a **full-view sharing link** in Proton Calendar web settings and add it using the Proton option. A paid Proton plan is required to create these links. Limited-view links may omit titles. The URL is a secret; anyone holding it may read the shared data. This app cannot change Proton's sharing/encryption model. Updates can be delayed by Proton; a broken link may need regeneration. A one-time ICS download is not a live subscription.

**ICS:** enter the private HTTPS feed URL. When reminder information is absent, local fallback reminders apply. Explicit disabled Google reminders and non-popup reminder channels do not generate duplicate local popups.

## Privacy and operation

See [docs/PRIVACY.md](docs/PRIVACY.md) for the threat model, vault/key lifecycle, OS notification limitations, and recovery. When native system-lock detection is unavailable, the app uses conservative focus-loss locking and reports this visibly. An explicit Google sign-in flow temporarily exempts expected browser focus changes; actual detected OS locking and idle locking remain active.

Closing the window quits unless **Keep running in background** is enabled. The tray menu can reopen the window, lock the vault, or quit. Idle locking still applies in the background. Existing OS notification history may retain delivered titles/times even after locking; privacy mode limits future payloads, and removal is best effort.

Encrypted backups include cached data and preferences, not OS-stored credentials. Restore into a fresh vault, then reconnect accounts. Losing every valid passphrase/unlocking method makes local data unrecoverable. Full-disk encryption remains valuable for swap and OS-owned data.

## Repository

| Path                        | Responsibility                                                          |
| --------------------------- | ----------------------------------------------------------------------- |
| `apps/desktop`              | React interface and Tauri desktop integration                           |
| `crates/calendar-core`      | UI-independent event model, recurrence, reminder policy                 |
| `crates/calendar-providers` | Versioned read-only provider contract and adapters                      |
| `crates/calendar-storage`   | SQLCipher repository, key wrapping, backups and receipts                |
| `docs`                      | Specification, architecture, privacy, validation, implementation status |

The Rust workspace defaults to the core crates; the desktop shell is checked separately. The core/provider/storage crates do not depend on Tauri or React, leaving a reuse path for future mobile clients. Included adapters are compiled and reviewed with the app; independently installed plugins are outside version 1.

## Checks

```sh
cargo test --locked
cargo check --locked -p read-only-calendar
cargo fmt --all -- --check
npm run build
npm test
```

CI runs the core/interface checks and a desktop build matrix for Linux, macOS, and Windows. A workflow definition is not evidence that those platforms have passed: inspect the actual run. Native notifications, OS keychain/session events, real Google consent and Proton feed updates need hands-on verification as documented.

## License

GPL-3.0-or-later for this project's original code. See [LICENSE](LICENSE). Commercial redistribution is permitted subject to GPL requirements. Third-party dependencies retain their own licenses. The MIT/MPL discussion did not change the repository's GPL license.
