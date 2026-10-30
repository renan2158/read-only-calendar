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

## New team member? Start here

1. Install the shared tools and the prerequisites for your operating system below.
2. Clone the repository and install the project dependencies.
3. Start the desktop app and create your local encrypted vault.
4. Connect a calendar when you are ready; provider accounts are only needed for integration testing.
5. Run the checks before opening a pull request. See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution rules.

### Shared development tools

| Tool                   | Required version / purpose                                                     | Installation                                                 |
| ---------------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| Git                    | Clone the repository and contribute changes                                    | [Git downloads](https://git-scm.com/downloads)               |
| Node.js and npm        | **Node.js 24 LTS**, with its bundled npm; frontend and Tauri CLI               | [Node.js downloads](https://nodejs.org/en/download)          |
| Rust, Cargo and rustup | Current **stable** Rust, at least **1.90**; compile the core and desktop shell | [Rust installation](https://www.rust-lang.org/tools/install) |
| rustfmt                | Format Rust code and run the formatting check                                  | `rustup component add rustfmt`                               |
| Native build tools     | Compile Tauri, SQLCipher and vendored OpenSSL                                  | Follow your OS section below                                 |

Internet access is needed for the first dependency download and provider connections. Run the desktop app in a graphical desktop session. The first Rust build compiles native libraries and can take several minutes; later builds reuse the cache. Our CI packaging targets are Windows x64, Linux x64, and both Apple Silicon and Intel macOS; this is not a guarantee of support for every older OS release.

After installing Rust, select stable and add the formatter:

```sh
rustup update stable
rustup default stable
rustup component add rustfmt
```

On **Windows**, use `rustup default stable-msvc` instead of `rustup default stable`. Reopen your terminal after installing tools so that their executables are on `PATH`.

Confirm the shared tools are available before continuing:

```sh
git --version
node --version
npm --version
rustc --version
cargo --version
```

### Linux prerequisites

Ubuntu **24.04** is the Linux build environment used by CI. On Ubuntu 24.04, install:

```sh
sudo apt-get update
sudo apt-get install -y \
  git curl wget file ca-certificates \
  build-essential make perl pkg-config \
  libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  libssl-dev libdbus-1-dev libxdo-dev \
  patchelf xdg-utils \
  gnome-keyring libpam-gnome-keyring
```

| Packages / service                                           | Why they are needed                                               |
| ------------------------------------------------------------ | ----------------------------------------------------------------- |
| `build-essential`, `make`, `perl`, `pkg-config`              | C/C++ compiler, linker, native builds and library discovery       |
| `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`                      | GTK desktop integration and the WebKitGTK **4.1** webview         |
| `libayatana-appindicator3-dev`, `librsvg2-dev`, `libxdo-dev` | Tray integration, SVG support and Tauri system dependencies       |
| `libssl-dev`, `libdbus-1-dev`                                | Native dependency headers for crypto and D-Bus integrations       |
| `patchelf`, `xdg-utils`                                      | Linux packaging and desktop utilities                             |
| `gnome-keyring`, `libpam-gnome-keyring`                      | One way to provide and unlock the Secret Service credential store |

A running **D-Bus user session**, an unlocked **Secret Service** provider, and a desktop notification service are required for credentials and notifications. GNOME commonly supplies these already. If your desktop uses another Secret Service provider, keep it rather than running competing keyrings. Minimal window managers, containers and SSH sessions may need additional desktop-session setup. Installing keyring packages alone does not start or unlock a keyring; sign out and back in if needed and check your desktop's keyring configuration.

Other distributions use different package names. Follow [Tauri's Linux prerequisite guide](https://v2.tauri.app/start/prerequisites/#linux), adding the equivalent compiler/make/Perl, D-Bus development, packaging and Secret Service dependencies above. This repository's copy-and-paste Linux setup is for Ubuntu 24.04.

### macOS prerequisites

1. Install Xcode Command Line Tools, then complete any installation/license prompts:

   ```sh
   xcode-select --install
   ```

2. Check the native tools are available:

   ```sh
   xcode-select -p
   clang --version
   make --version
   perl --version
   ```

The compiler, SDK, make and Perl are used to build the Rust/native dependencies. If Perl is missing, install it through your preferred package manager before compiling. The app uses the macOS system webview and Keychain. Homebrew and a separate SQLCipher/OpenSSL installation are not required by the default bundled build. Full Xcode and mobile SDKs are only needed when mobile support is implemented.

### Windows prerequisites

1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) (or Visual Studio) with **Desktop development with C++**, the **MSVC C++ build tools**, and a **Windows SDK**. This supplies the native compiler, linker and `nmake`.
2. Install the [Microsoft Edge WebView2 Evergreen Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) if it is not already present.
3. Install [Strawberry Perl](https://strawberryperl.com/) and ensure `perl` is on `PATH`; bundled OpenSSL needs Perl during compilation. MSVC remains the project's compiler even if your Perl distribution includes other tools.
4. Select the Rust MSVC toolchain with `rustup default stable-msvc`. Use **Developer PowerShell for Visual Studio** / a native-tools developer terminal if the compiler is not found.
5. To build `.msi` installers, enable the Windows **VBScript optional feature**. NSIS/WiX packaging tools may be downloaded by Tauri on the first installer build. See [Tauri's Windows guide](https://v2.tauri.app/start/prerequisites/#windows).

Windows Credential Manager supplies the OS credential store. Use a normal signed-in desktop session. Prefer the installed application when verifying Windows notifications, so its notification registration is available.

### Clone, install and run

Until PR #1 is merged, the application source is on **`feat/calendar-v1`**. From a terminal:

```sh
git clone https://github.com/renan2158/read-only-calendar.git
cd read-only-calendar
git switch feat/calendar-v1
npm ci
npm run desktop -- dev
```

Run these commands from the **repository root**. `npm ci` installs the frontend libraries and local Tauri CLI using `package-lock.json`. The first desktop launch downloads and compiles Rust dependencies using Cargo. Commit the existing lockfiles when dependency changes are intentional; do not regenerate them just to get started.

The desktop window should open after compilation. Create a vault with a passphrase of at least **12 characters**, then use **Connect calendar**. Store your passphrase safely: there is no password-reset server. Provider credentials and subscription links are encrypted with a vault-derived key before being saved to the OS credential store; a missing/locked credential store causes an error, never a plaintext fallback.

`npm run dev` starts only the frontend server at `http://127.0.0.1:1420`; real vault/provider operations require the Tauri desktop shell. No demo data or fake success state is used by the application.

### Libraries installed by the project

Install the OS packages above yourself. **Do not install these application libraries individually or install a global Tauri CLI**: npm and Cargo resolve the versions from the manifests and lockfiles.

| Area                           | Main direct dependencies                                                                            | Installed by |
| ------------------------------ | --------------------------------------------------------------------------------------------------- | ------------ |
| Interface                      | React, React DOM, TypeScript, Lucide icons; Tauri JS API and notification bindings                  | `npm ci`     |
| Frontend tooling               | Vite, React Vite plugin, Tauri CLI, Prettier                                                        | `npm ci`     |
| Interface tests                | Vitest, jsdom, React Testing Library, jest-dom                                                      | `npm ci`     |
| Desktop shell                  | Tauri 2; notification, dialog and opener plugins; `keyring`; Windows session APIs via `windows-sys` | Cargo        |
| Event model and recurrence     | `chrono`, `chrono-tz`, `rrule`, `ical`                                                              | Cargo        |
| Provider transport and parsing | `reqwest` with rustls TLS, `url`, `roxmltree`, `base64`                                             | Cargo        |
| Encrypted local storage        | `rusqlite` with **bundled SQLCipher and vendored OpenSSL**, `argon2`, `aes-gcm`, `hkdf`             | Cargo        |
| Shared utilities               | `serde`, `serde_json`, `thiserror`, `uuid`, `zeroize`, `sha2`, `hex`, `getrandom`, `tempfile`       | Cargo        |

For the complete declared dependency lists and version constraints, see the [root npm manifest](package.json), [desktop npm manifest](apps/desktop/package.json), [Rust workspace](Cargo.toml), and the [core](crates/calendar-core/Cargo.toml), [providers](crates/calendar-providers/Cargo.toml), [storage](crates/calendar-storage/Cargo.toml), and [desktop shell](apps/desktop/src-tauri/Cargo.toml) manifests. Exact resolved versions live in [package-lock.json](package-lock.json) and [Cargo.lock](Cargo.lock).

SQLite/SQLCipher is embedded; **no database server, Docker, backend service or app account is required**. No `.env` file or provider credentials are required to compile the app or run fixture-based tests. Configure providers through the desktop interface as described below.

## Build executable files

After `npm ci`, run **`npm run build:desktop`** to build release executables and installers for your current OS. Explicit aliases are `npm run build:windows`, `npm run build:macos`, and `npm run build:linux`; run each on its matching OS.

The **Build installers** GitHub Actions workflow builds all platforms, including Intel and Apple Silicon macOS, and uploads downloadable installer artifacts. See [docs/BUILDING.md](docs/BUILDING.md) for commands, output paths, prerequisites, signing and downloading artifacts.

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

## Everyday commands and checks

Run commands from the repository root:

| Command                                      | Purpose                                                           |
| -------------------------------------------- | ----------------------------------------------------------------- |
| `npm run desktop -- dev`                     | Launch the desktop app with frontend development reload           |
| `npm run dev`                                | Frontend server only; native operations require Tauri             |
| `npm run check`                              | TypeScript checks                                                 |
| `npm run build`                              | TypeScript checks and production frontend bundle                  |
| `npm test`                                   | Interface and date tests                                          |
| `npm run test:build`                         | Build-command helper tests                                        |
| `cargo test --locked`                        | Core, provider and storage tests; workspace default members       |
| `cargo check --locked -p read-only-calendar` | Check the native desktop shell; requires OS development libraries |
| `cargo fmt --all -- --check`                 | Check Rust formatting                                             |
| `npm run desktop -- build --no-bundle`       | Compile a release desktop executable without installers           |
| `npm run build:desktop`                      | Compile the release app and native installers                     |

Before opening a pull request, run:

```sh
npm run test:build
cargo test --locked
cargo check --locked -p read-only-calendar
cargo fmt --all -- --check
npm run build
npm test
```

CI runs the core/interface checks and packages downloadable installers for Linux, macOS, and Windows. A workflow definition is not evidence that those platforms have passed: inspect the actual run. Native notifications, OS keychain/session events, real Google consent and Proton feed updates need hands-on verification as documented.

## Setup troubleshooting

| Symptom                                               | What to check                                                                                                                  |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `node`, `npm`, `cargo` or `perl` not found            | Install the matching tool, reopen your terminal, and check its version. On Windows, check the MSVC developer terminal as well. |
| WebKit/GTK or `pkg-config` build errors               | Install the Linux development packages above. WebKitGTK 4.0 does not replace the required 4.1 package.                         |
| `link.exe`, C compiler, SDK or OpenSSL build failures | Check the native compiler, make/`nmake` and Perl. On Windows use MSVC, not a GNU Rust target.                                  |
| Credential-store / Secret Service errors              | Unlock the OS keyring; on Linux check the D-Bus user session and a running Secret Service provider.                            |
| Frontend cannot call native commands                  | Start with `npm run desktop -- dev`, not just `npm run dev`.                                                                   |
| Port 1420 already in use                              | Stop the previous development server before launching another one.                                                             |
| Missing notifications                                 | Check OS permission/Do Not Disturb, keep the app running and vault unlocked, and check calendar synchronization.               |
| AppImage build fails without FUSE                     | See the `APPIMAGE_EXTRACT_AND_RUN=1` option in [BUILDING.md](docs/BUILDING.md).                                                |

When reporting a problem, include your OS/architecture, tool versions, command and a sanitized error excerpt. Keep tokens, private feed URLs, event contents, vaults and backups out of issues and logs. See [SECURITY.md](SECURITY.md) for security reports.

## License

GPL-3.0-or-later for this project's original code. See [LICENSE](LICENSE). Commercial redistribution is permitted subject to GPL requirements. Third-party dependencies retain their own licenses.
