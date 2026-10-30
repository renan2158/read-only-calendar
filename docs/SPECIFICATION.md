# Privacy-first Calendar — Version 1 specification

Version: 1.0 • Date: 2026-10-29 • Status: agreed requirements consolidated; feasibility gates pending evidence.

The working name is descriptive, not a selected product name. This document supersedes earlier discussion of a writable first release. Implementation was subsequently authorized by the user on 2026-10-29. The feasibility and platform gates below remain evidence requirements for release; incomplete live-account checks do not block the explicitly requested development build.

## 1. Product and boundaries

**Goal:** a privacy-first, read-only desktop calendar that combines calendars from multiple providers and delivers event-start notifications and reminders. New providers must be straightforward to add through a documented, versioned adapter contract.

| ID   | Requirement                                                                                                                                                                                                                                          |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S-01 | Ship on Linux, macOS, and Windows. Keep the Rust domain and synchronization core independent of desktop UI/lifecycle APIs to support future mobile clients. Mobile is outside version 1.                                                             |
| S-02 | Use Tauri for the desktop shell, Rust for core services, React/TypeScript for the interface, and SQLCipher-encrypted SQLite for local data.                                                                                                          |
| S-03 | Connect directly to calendar providers; no mandatory application account, central event relay, or hosted synchronization backend.                                                                                                                    |
| S-04 | Include Google Calendar, generic CalDAV, and ICS subscriptions. Proton support uses only its official shared calendar links through the ICS adapter.                                                                                                 |
| S-05 | Calendar content is read-only, including invitations. No create, edit, delete, move, RSVP, or invitation-sending action. Local display/reminder preferences are editable.                                                                            |
| S-06 | Synchronize each calendar with its own provider and display them together. Do not copy or propagate events between providers.                                                                                                                        |
| S-07 | Offline mode displays encrypted cached events and supports local alerts while unlocked; show last successful synchronization and stale/error status.                                                                                                 |
| S-08 | Use a copyleft open-source license. GPL-3.0-or-later is the implementation license, consistent with the existing repository license. Commercial redistribution remains permitted under the license. Earlier noncommercial restriction is superseded. |

Tasks, planning/AI, scheduling links, contacts, mobile delivery, cloud accounts, external installable plugins, event writing, pending-write queues, and edit-conflict resolution are outside version 1. The architecture may support future versions, but must not expose write capabilities now.

## 2. User experience and event representation

| ID   | Requirement                                                                                                                                                                                                                                                    |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| U-01 | Provide day, week, month, and agenda views and search within unlocked encrypted local data.                                                                                                                                                                    |
| U-02 | Support multiple accounts/calendars, visibility controls, colors, selected display time zone, and locale-aware date/time formatting.                                                                                                                           |
| U-03 | Display title, time, all-day status, description, location, organizer/attendees and response information where supplied. Missing or private data must not be fabricated.                                                                                       |
| U-04 | Represent recurring series, recurrence exceptions, exclusions, moved occurrences, cancellations, dates without times, floating times, and original time-zone information. End dates on all-day events are exclusive.                                           |
| U-05 | Preserve unsupported source fields in encrypted raw records; no silent data loss during refresh. Use provider/account/calendar/resource identity plus recurrence identity. Identical titles or UIDs in different calendars are not sufficient to merge events. |
| U-06 | Disable remote resource fetching in event descriptions. Render sanitized content without scripts, embedded remote images, or automatic external requests. External navigation is explicit.                                                                     |
| U-07 | Show actionable account/sync errors using safe messages. Retain the last complete snapshot on transient failure or malformed/partial feed; mark it stale. Do not treat a failed fetch as an empty calendar.                                                    |
| U-08 | Support keyboard navigation and accessible labels/contrast; preserve system text scaling and light/dark preferences. Details are implementation acceptance criteria, not a new feature expansion.                                                              |

## 3. Notifications and reminders

| ID   | Requirement                                                                                                                                                                                                                                                                                                                         |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| N-01 | Notify at the start of an eligible event and at each configured reminder time. Notification delivery is best effort and depends on the app/OS being active, permissions, an unlocked vault, and sufficiently current source data.                                                                                                   |
| N-02 | Default content includes event title and start time. Example: “Team meeting — starts in 10 minutes, 14:00.”                                                                                                                                                                                                                         |
| N-03 | Optional “Hide event details” conceals the event title and other identifying text while keeping the time visible. Example: “Upcoming event — starts in 10 minutes, 14:00.” This applies to alert summaries and app-provided notification payloads.                                                                                  |
| N-04 | Use provider display/popup reminders when available. If the source omits reminder information, offer a configurable local fallback, initially 10 minutes before. An explicitly empty/disabled source reminder setting is distinct from missing/unsupported information. Do not duplicate server email/SMS channels as local popups. |
| N-05 | For all-day events, use a configurable local notification time, initially 09:00 in the selected display time zone; label the event “All day.” Apply reminder offsets relative to this local anchor.                                                                                                                                 |
| N-06 | Deduplicate by account, calendar, stable occurrence identity, and delivery instant. If a zero-offset reminder and start alert coincide, emit one alert. Persist delivery receipts encrypted so restarting does not repeat delivered alerts.                                                                                         |
| N-07 | Refreshes reschedule changed occurrences and cancel removed/cancelled occurrences. A failed refresh must not erase reminders derived from the last good snapshot. Recheck current state immediately before delivery.                                                                                                                |
| N-08 | After sleep, restart, or unlock, show one summary of recently missed eligible alerts rather than a burst. “Recent” duration and wording remain tuning defaults; do not notify for every historic event.                                                                                                                             |
| N-09 | Clicking a notification opens event details and requires unlock if needed. Notification activation identifiers must be opaque and contain no event text.                                                                                                                                                                            |
| N-10 | Offer an explicit keep-running-in-background setting. Closing the window while enabled can retain an unlocked vault; a full quit stops background work.                                                                                                                                                                             |
| N-11 | A locked vault pauses provider sync and event-based notifications. Cancel future app-owned OS schedules on lock; do not persist plaintext future notification payloads to bypass locking. Already-delivered notifications may remain in OS history; minimize/remove those best effort without claiming guaranteed erasure.          |
| N-12 | Handle recurring occurrences, DST, system clock changes, display time-zone changes, cancellation, hidden calendars, and denied OS permissions deliberately. Hidden calendars suppress notifications by default, with a separate explicit notification toggle if needed.                                                             |

## 4. Privacy, encryption, and key lifecycle

**Threat model:** protect local data at rest, copied database/backup files, accidental log disclosure, and unnecessary network disclosure. Data must be decrypted in memory while used. Local encryption does not change a provider's access to its own calendar data, prevent malware in an unlocked user session, hide traffic metadata, or guarantee memory/swap/OS-notification erasure.

| ID   | Requirement                                                                                                                                                                                                                                                                                                                                                |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P-01 | Encrypt all local calendar data by default with SQLCipher: events, raw source payloads, search indexes, synchronization cursors, preferences containing private information, reminder schedules, and delivery receipts. Encryption is mandatory.                                                                                                           |
| P-02 | Generate a random 256-bit data key. Never hardcode it, derive it from device identifiers, or save it next to the database in plaintext. Verify the SQLCipher library and encryption settings at startup; fail closed rather than create a plain SQLite database.                                                                                           |
| P-03 | Default to passphrase unlock. Protect/wrap the data key using a reviewed authenticated-encryption scheme and a memory-hard passphrase KDF; Argon2id is the prototype candidate. Store salt, nonce, algorithm/format version, and bounded KDF parameters with the encrypted key envelope.                                                                   |
| P-04 | Store provider credentials through OS credential storage. In passphrase mode, wrap secret values with a vault-derived secret key before placing them in that storage, so an unlocked OS keychain does not bypass application locking. Tokens are accessible only while unlocked. No silent plaintext/file/mock fallback.                                   |
| P-05 | Device-based automatic unlock is an explicit convenience option. Its protected data key can be stored in the OS credential store, and its weaker independence from the OS session must be explained. Linux must report a missing/locked credential service rather than silently weaken protection.                                                         |
| P-06 | Lock on OS-session lock by default, plus manual lock. Lock hides event views, stops/joins workers, closes connections, invalidates in-flight results and notification jobs, and releases keys/decrypted caches as far as practical. Screen hiding alone is not vault locking. Idle locking is configurable; its initial timeout remains a tuning decision. |
| P-07 | Keep temporary SQL stores in memory; verify encrypted WAL/journals and backup behavior. Enable relevant memory protections supported by the chosen library; do not promise all memory is locked or zeroized. Disable core dumps where practical.                                                                                                           |
| P-08 | No event titles, times, attendees, descriptions, raw source responses, account emails, tokens, feed URLs, SQL parameters, or key material in logs. Log safe categories/codes and opaque local IDs. No analytics or automatic crash uploads. Dependencies must not enable verbose HTTP/SQL/OAuth tracing.                                                   |
| P-09 | Require HTTPS and certificate verification for remote endpoints. Credentials must not be forwarded across origins. Disable uncontrolled redirects and reject downgrade redirects. User-configured local/LAN services require an explicit, narrowly scoped policy; test HTTP servers bind to loopback only.                                                 |
| P-10 | Treat ICS URLs, including Proton shared links, as credentials. Encrypt/store them as secrets; never send them to our servers or include them in logs, filenames, exports of settings, or crash reports.                                                                                                                                                    |
| P-11 | Provide encrypted backup/restore and passphrase change without plaintext intermediary files. Backup recovery must be portable and must not depend solely on the original device keychain. Credentials should be reauthorized after restore rather than exported by default.                                                                                |
| P-12 | Explain that loss of every usable unlocking/recovery method makes local data unrecoverable. Cached provider data may be fetched again; local settings/delivery history require backup. Account removal clears local records and credentials, cancels notifications, and explains backup/OS-history limitations.                                            |
| P-13 | Release signing, authenticated updates, dependency/license review, and explicit privacy documentation are release gates. Do not execute remotely supplied feed content or load arbitrary plugins.                                                                                                                                                          |

## 5. Provider integrations

### Google

Use installed-app OAuth in the system browser with PKCE S256, state validation, loopback callback bound to loopback only, and a bounded callback lifetime. Desktop OAuth client configuration is public configuration, not an embedded secret. Scope requests must remain read-only. Initial candidate: `calendar.calendarlist.readonly` plus `calendar.events.readonly`; add another read-only scope only for a demonstrated API requirement. Public distribution needs the applicable Google consent/verification setup.

Read calendar list and event pages. Stage paginated changes and commit event data and the next cursor in one encrypted transaction only after all pages succeed. Handle cancellations/deletions, expiry, refresh, revocation, 429/5xx with backoff, and invalid `syncToken` (410) by safely building a replacement snapshot. Retain old data during failed resync. Preserve query shape across incremental pages; do not combine prohibited time filters with `syncToken`. Recurrence/expansion strategy must be demonstrated for the chosen bounded display/notification horizon before shipping. Poll directly; do not introduce a required public webhook server.

### Generic CalDAV

Support principal and calendar-home discovery, calendar listing, privileges, resource ETags, calendar-query/calendar-multiget reads, and RFC 6578 collection synchronization when advertised. Fall back to a complete ETag inventory when unavailable. A generic capability claim requires at least two independent server implementations. HTTPS credentialed live reads must be tested. No `PUT`, `DELETE`, `MKCALENDAR`, `PROPPATCH`, scheduling `POST`, or event `PATCH` from the application. Read operations can use `OPTIONS`, `PROPFIND`, `REPORT`, and `GET`.

Account credentials might themselves allow writing, but the v1 adapter and transport cannot expose write operations. Authentication mechanisms vary by server; document supported mechanisms rather than promise arbitrary providers.

### ICS and official Proton sharing

Subscribe to a URL and refresh it; importing a one-time ICS file alone is not ongoing synchronization. Parse iCalendar with size, recurrence-expansion, and processing limits. Respect HTTP validators when supplied; use safe polling/backoff and retain previous data on invalid responses. Reconcile complete successful snapshots, not partial failed downloads.

Proton is an onboarding preset over this same adapter, not a private API integration. A paid Proton plan is currently required to create shared links. Full-view links are needed for titles; limited-view links may supply busy-only information. Do not claim true end-to-end confidentiality for a decrypted shared feed. Reminder fields may be absent; use the local fallback. Proton documents subscribed-link update issues; expose freshness and instructions to regenerate a broken link without logging it.

## 6. Extensible architecture and contract v1

Keep these boundaries: React presentation → narrowly scoped Tauri commands/events → Rust application services → provider adapters and encrypted repository. OS credential storage, notifications, clocks, lifecycle, and network access are platform interfaces.

Included adapters are reviewed and compiled with the application. Contract `1.x` provides provider identity, supported auth types, capabilities, account/calendar discovery, cancellable read synchronization, opaque/versioned cursors, typed safe errors, and reminders represented as supported-present / supported-disabled / unknown. Outputs include stable resource identity, original occurrence identity, upserts/tombstones or complete snapshots, and source extensions. Core owns retries, storage, recurrence normalization, scheduling, and logging policy. Capabilities cannot enable writing in v1.

Adapters must not call React/Tauri APIs, hold DB keys, write files, or manage their own notification queues. Define page completion/atomicity and cancellation semantics. Additive changes are minor revisions; incompatible changes require a major version and cursor migration/reset rules. Provide a reusable conformance test suite and fixture guidance. Future write interfaces should be a separate later contract extension, not dormant writable endpoints in v1.

Mobile should reuse provider/domain/storage code, but has separate credential, lifecycle, and notification implementations. No promise of desktop-equivalent mobile background execution.

## 7. Validation evidence required before release

| Gate               | Required evidence to pass                                                                                                                                                                                                                                                                                   |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| G-01 Google        | Actual test-account OAuth/refresh, paginated read sync, recurring/cancelled instances, incremental updates, 410 recovery, safe log handling; protocol fixtures alone are partial evidence.                                                                                                                  |
| G-02 CalDAV        | Read-only discovery, calendar/event reads, changes/deletions and sync-token or ETag fallback against two independent implementations; TLS/auth in a live test environment. One loopback server is partial evidence.                                                                                         |
| G-03 Proton/ICS    | Official-sharing documentation plus real paid-account full-view feed read, updates, cancellations, recurrence/time zones, link revocation and reminder availability inspection. Synthetic ICS checks are partial evidence.                                                                                  |
| G-04 Storage       | Actual SQLCipher encrypted create/reopen, no-key/wrong-key failure, WAL checks, integrity/corruption rejection, encrypted backup/restore, passphrase change, lock rejection, plus actual Rust integration and OS keychain/lifecycle checks on all three desktops. Library-level tests are partial evidence. |
| G-05 Notifications | Separate pre-release gate: native delivery, permissions, background/quit/lock/sleep/resume, missed summaries, deduplication, privacy wording and notification clicks on all three OSes. No full shell is required for protocol spikes.                                                                      |

No gate may be called passed solely because documentation says a feature exists. Record DOCUMENTED, FIXTURE-TESTED, LOCALLY-INTEGRATED, LIVE-ACCOUNT-TESTED, PLATFORM-TESTED, or BLOCKED for each piece of evidence. Use synthetic data in automated tests and deliverables. Do not request passwords/tokens in chat; real credentials stay on the user's device.

## 8. Acceptance and rollout

Required release behavior includes restart-safe notifications, source-change rescheduling, no event writes, correct DST/recurrence handling, no sensitive diagnostics, failed-sync snapshot retention, fail-closed encryption, reproducible builds, signed update artifacts, and accessible UI. Measure performance using realistic multi-calendar fixtures before setting numerical performance targets. Exact packaging formats, minimum OS versions, missed-alert window, poll intervals and idle-lock timeout are implementation defaults to document before beta, not claims already agreed.

Sequence: specification → focused prototypes/evidence → Rust core/adapter implementation → complete blocked live/platform release gates → Tauri/React shell → native notification validation → privacy/release review. Do not interpret prototype success as a production security audit.

## 9. Primary references (checked 2026-10-29)

1. Google read-only scopes: https://developers.google.com/workspace/calendar/api/auth
2. Google installed-app OAuth: https://developers.google.com/identity/protocols/oauth2/native-app
3. Google sync/cursor handling: https://developers.google.com/workspace/calendar/api/guides/sync
4. Google event/reminder representation: https://developers.google.com/workspace/calendar/api/v3/reference/events
5. CalDAV: https://www.rfc-editor.org/rfc/rfc4791
6. WebDAV synchronization: https://www.rfc-editor.org/rfc/rfc6578
7. Official Proton calendar sharing: https://proton.me/support/share-calendar-via-link
8. SQLCipher design: https://www.zetetic.net/sqlcipher/design/
9. SQLCipher API: https://www.zetetic.net/sqlcipher/sqlcipher-api/
10. SQLCipher licensing: https://www.zetetic.net/sqlcipher/license/
11. Argon2: https://www.rfc-editor.org/rfc/rfc9106
12. Open Source Definition: https://opensource.org/osd
13. GPL v3: https://www.gnu.org/licenses/gpl-3.0.html
