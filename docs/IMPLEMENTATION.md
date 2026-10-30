# Version 0.1.0 implementation and verification

Date: 2026-10-29. The user authorized implementation after the initial prototypes. This is a **development first version**, with working application source and a successfully built Linux desktop binary, not a claim of production security certification or completed live-provider/platform testing.

## Implemented

- One monorepo: three Tauri-independent Rust crates, Tauri desktop integration, React/TypeScript frontend, locked Rust/npm dependencies, GPL-3.0-or-later, architecture/privacy/specification documents and a desktop CI matrix.
- Encrypted vault creation/unlock; random data key, Argon2id/AES-GCM envelope, atomic key-envelope replacement, passphrase change, HKDF/AES-GCM credential wrapping and explicit OS credential stores. Optional device auto-unlock, no plaintext/mock fallback.
- SQLCipher repository, encrypted snapshot/FTS/cursors/preferences/receipts, encrypted backup export with an independent wrapped key, restore into a fresh vault and credential reauthorization afterward.
- Google installed-app browser OAuth with PKCE/state/loopback timeout, read-only scopes, refresh, paginated initial/incremental reads, safe staged cursor updates, cancellations/recurrence exceptions and expired-cursor rebuild.
- HTTPS CalDAV principal/home/calendar discovery, ETag inventory, multiget, collection synchronization tokens, invalid-token fallback and safe XML/status parsing. Initial authentication supports Basic/app passwords; cross-origin redirects and invalid certificates are refused.
- HTTPS ICS/Proton subscriptions with conditional HTTP validators and full-snapshot reconciliation; no private Proton API, account password or visitor write path.
- Bounded IANA-zone recurrence expansion, all-day/exclusive end handling, moved/cancelled exceptions, DURATION, local reminders, start alerts and encrypted delivery receipts. Unsupported/custom timezones or unsupported recurrence/alarm constructs fail visibly instead of silently dropping information.
- Native notification integration with opaque event activation identifiers, start/reminder deduplication, late-alert summaries, visible time in privacy mode, tray/background/quit, idle/manual/session locking, cancellation generations, and conservative focus-loss fallback if OS lock detection is unavailable.
- Month/week/day/agenda, calendar toggles/colors, search, event details with plain-text descriptions, connection forms, preferences and encrypted backup/password controls. The UI has no real-data browser mode, remote description resources, app-account dependency or plaintext web storage cache.

## Checks executed here

| Check                                            | Result                                                                      |
| ------------------------------------------------ | --------------------------------------------------------------------------- |
| `cargo test --locked` (default workspace crates) | 36 passing Rust tests, including one abrupt-exit helper test                |
| `cargo build --locked -p read-only-calendar`     | Linux desktop binary compiled and linked successfully                       |
| `cargo check -p read-only-calendar`              | Passed on Linux                                                             |
| `cargo fmt --all -- --check`                     | Required and checked before upload                                          |
| `npm run build`                                  | TypeScript checking and Vite production build passed                        |
| `npm test`                                       | 9 passing interface/date tests                                              |
| `npm audit --omit=dev`                           | No production dependency vulnerabilities reported at the time of this check |

Rust tests include SQLCipher wrong/no-key and plaintext rejection, encrypted database/WAL/FTS, backup/restore, wrapping/tamper rejection, passphrase change, crash/transaction recovery, persistent receipts, recurrence/DST/moved/cancelled instances and reminder privacy/deduplication. Interface tests check calendar-date boundaries, exclusive all-day dates, cleared event details after locking, descriptions rendered without remote elements, and privacy wording.

Synthetic crypto negative tests intentionally produce fixed integrity diagnostics. React may emit asynchronous-test scheduling warnings; the tests pass, but those messages are not evidence of a native-platform test. No personal calendar data or real credentials were used.

The Linux shell was compiled against locally prepared, checksum-verified Ubuntu development libraries because system installation was unavailable. This validates native compilation/linking, not an installed end-user environment. Browser/native GUI launch in this execution environment is blocked by Unix-socket restrictions; no screenshot or GUI smoke test is reported as passing. Unit interface tests and TypeScript/build verification did run.

## Remaining release validation and limitations

1. Real Google consent/refresh/revocation/source-change behavior. The user must provide a local Desktop OAuth client configuration or the maintainer must configure and verify an application-owned client. No Google connection was fabricated.
2. Real Proton full-view shared-feed update/cancellation/revocation and reminder-field availability. Proton's link sharing remains read-only and may be delayed.
3. Live HTTPS/authenticated CalDAV behavior of the production Rust adapter against two independent implementations. The older Python probes exercised Radicale/Xandikos, but do not substitute for this Rust adapter's live tests.
4. Native keychain, OS session-lock detection, notification permissions/clicks/history, sleep/resume and background operation on Linux, macOS and Windows. Windows/macOS build jobs are configured, not presumed passed.
5. Calendar edge cases: custom VTIMEZONE definitions without an IANA equivalent, RECURRENCE-ID `RANGE=THISANDFUTURE`, absolute/end-relative/sub-minute alarms are conservatively rejected by the current ICS normalization. The last successful snapshot is retained with an error. These need explicit handling for providers using those constructs.
6. Reminder delivery is best effort. The notification library dispatches to the OS asynchronously; preparation success is not a guarantee of delivery or the user's seeing an alert. Windows notification activation is limited by the underlying notification implementation, especially after a toast moves to notification history.
7. OS-owned notification history and memory/swap cannot be guaranteed erased. Device unlock deliberately relies on the logged-in OS session. Native-file-dialog focus changes may activate conservative fallback locking on systems without a session-lock monitor; keep that platform check on the release list.
8. Code-signing/notarization, signed update distribution and full dependency/license/security review remain release tasks. No unsigned automatic updater or required update server is enabled. This build should not be represented as a hardened stable release.

The continuous-integration workflow builds the desktop on all three target operating systems and runs local tests. Inspect actual workflow results after upload; configuration alone is not a passing result. The original feasibility report is retained as historical evidence, clearly separate from these implementation results.
