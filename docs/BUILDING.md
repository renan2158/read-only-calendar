# Build executables and installers

Run at the repository root after checking out the application branch (`feat/calendar-v1` while PR #1 is unmerged):

```sh
npm ci
npm run build:desktop
```

`build:desktop` detects your OS, builds the React frontend and Rust executable in release mode, and creates native bundles using the locally installed Tauri CLI. The existing `npm run build` remains frontend-only.

| Build machine | Command                 | Bundles                                  |
| ------------- | ----------------------- | ---------------------------------------- |
| Windows       | `npm run build:windows` | NSIS setup `.exe` and WiX `.msi`         |
| macOS         | `npm run build:macos`   | `.app` application and `.dmg` disk image |
| Linux         | `npm run build:linux`   | `.deb` package and `.AppImage`           |

Each local command requires the corresponding OS and native toolchain. To build all platforms from one computer, use GitHub Actions below.

## Output paths

- Executable: `target/release/read-only-calendar.exe` on Windows; `target/release/read-only-calendar` on Linux/macOS.
- Installers: `target/release/bundle/`, under `nsis`, `msi`, `macos`, `dmg`, `deb`, or `appimage`.
- With `--target`, use `target/<triple>/release/bundle/`.
- Custom Cargo target/profile settings can change these locations. Tauri prints the actual paths.

Prefer the installed application on Windows for correct notification registration. Distribute a macOS `.app`/`.dmg` rather than its bare executable, because the bundle contains application resources.

## Prerequisites

Install Node.js 24, current stable Rust, and [Tauri's platform prerequisites](https://v2.tauri.app/start/prerequisites/).

- Windows: Visual Studio C++ build tools, Windows SDK, WebView2, and native build tools/Perl for bundled OpenSSL. Installer generation may download NSIS/WiX tooling. MSI generation requires the Windows VBScript optional feature.
- macOS: Xcode command-line tools and the Rust targets you build.
- Linux: C/C++ build tools, make, Perl, pkg-config, WebKitGTK 4.1/GTK development libraries, Ayatana AppIndicator, librsvg, SSL/DBus development libraries, patchelf and xdg-utils. For Ubuntu:

```sh
sudo apt-get update
sudo apt-get install -y build-essential perl pkg-config libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libdbus-1-dev patchelf xdg-utils
```

AppImage bundling may need FUSE. If unavailable, set `APPIMAGE_EXTRACT_AND_RUN=1` for the build; the GitHub workflow does this automatically. A Linux build still requires compatible runtime libraries: an AppImage does not guarantee compatibility with every older distribution.

## Architecture and build options

On macOS, Tauri arguments are forwarded unchanged:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run build:macos -- --target aarch64-apple-darwin
npm run build:macos -- --target x86_64-apple-darwin
```

Inspect the command without compiling:

```sh
npm run build:desktop -- --dry-run
npm run build:desktop -- --help
```

## Build all platforms using GitHub Actions

The **Build installers** workflow (`.github/workflows/ci.yml`) builds Windows x64, Linux x64, Apple Silicon macOS and Intel macOS on suitable runners. Pushes and pull requests trigger it automatically. It uploads artifacts without publishing a GitHub release or changing updater configuration.

1. Open the repository's **Actions** tab and choose **Build installers**.
2. Open a completed run for the branch/commit you want.
3. Download an artifact: `read-only-calendar-windows-x64`, `read-only-calendar-linux-x64`, `read-only-calendar-macos-apple-silicon`, or `read-only-calendar-macos-intel`.
4. Extract its ZIP and use the installer inside. macOS app directories are also archived as `.app.tar.gz` to preserve permissions and symlinks.

Artifacts are retained for 14 days. Manual **Run workflow** becomes available once this workflow is on the default branch. After merging to `main`, an authenticated GitHub CLI can start all builds:

```sh
gh workflow run ci.yml --ref main
gh run list --workflow ci.yml
```

Inspect and download a selected run:

```sh
gh run watch RUN_ID
gh run download RUN_ID --dir artifacts
```

## Development signing

These commands produce development packages. On macOS the wrapper defaults to an ad-hoc identity (`-`) if `APPLE_SIGNING_IDENTITY` is unset; an explicitly provided identity is respected. Ad-hoc signing is not Developer ID certification or notarization. Windows packages are unsigned unless signing is configured. Public-distribution signing/notarization remains a separate release task. No signing secrets are embedded in source.
