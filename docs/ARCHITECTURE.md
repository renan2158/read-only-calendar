# Architecture and provider contract v1

A single repository coordinates one release across the Rust workspace and React application. `calendar-core` owns event identities, bounded recurrence, notification policy, and serializable data. `calendar-storage` owns the encrypted SQLite connection, FTS index, atomic snapshot writes, delivery receipts, and encrypted key envelopes/backups. `calendar-providers` implements the read-only contract and does not depend on storage or desktop APIs. Tauri orchestrates lifecycle, credentials, sync jobs, notifications, dialogs, and restricted IPC.

## Contract 1

Implement `CalendarProvider` in `crates/calendar-providers`. It receives an account identity, prior calendar/source records, display timezone, and shared cancellation token. It returns complete synchronized calendars with opaque cursors and normalized source events. The application commits the result transactionally only after all provider pages and validation complete. Failure preserves the previous account snapshot and records a safe error. A generation token rejects results after vault locking; account removal rejects old results for that account.

All version 1 adapters are read-only. The transport exposes `ReadMethod::{Get,Propfind,Report}` and cannot submit event writes. OAuth token POST requests are isolated to the pinned Google token endpoint. Authenticated discovery cannot change origin. HTTP redirects and certificate bypasses are disabled. Timeouts, cancellation, body/page/event limits and bounded retries protect the reader. New account credentials are serialized only in memory, wrapped by the storage layer, and placed in the OS credential store by the shell.

Provider-specific capabilities are expressed by the credential/provider variant and reminder state: unknown, explicitly disabled, or a set of offsets. Google supports its own OAuth path; CalDAV v1 supports Basic/app passwords; feeds need only their private URL. Future provider/auth additions should extend the documented contract without exposing raw secrets or SQL in IPC. Breaking cursor/model changes require a version increment and safe full resynchronization.

## Add an adapter

1. Add a provider module and implement `CalendarProvider` using the restricted transport and cancellation token.
2. Extend the protected credential enum and the `sync` dispatch. Do not derive Debug on credentials or log response bodies/errors containing URLs.
3. Map identity, recurrence exceptions, cancellation, permissions/reminder capabilities, raw extensions and cursor lifecycle deliberately.
4. Add a connection form with the minimum inputs; save secrets through the shell credential helper only.
5. Add protocol fixtures and transport/auth/error tests, then validate against a disposable real account. A fixture pass is not a live integration pass.

## Security boundaries

React receives display records, calendar preferences and account labels only while unlocked. Source URLs, sync cursors, raw provider payloads, DB keys and saved credentials are not returned by the view command. Event descriptions are displayed as text. UI lock events invalidate in-flight refreshes and clear rendered data. No localStorage/IndexedDB calendar cache is used.

Vault locking cancels the current provider generation, drops the database connection/key, prevents new notification work, hides UI state and requests best-effort removal of delivered notifications. A bounded in-flight HTTP request may take up to its timeout to exit; its result is discarded. Already allocated Rust/JavaScript strings and OS-owned notification buffers cannot be guaranteed erased. This limitation is part of the threat model rather than an unqualified memory-erasure claim.

## Future mobile clients

Keep domain, providers and storage independent of the shell. A mobile application can reuse them while implementing different notification, credential-store, lifecycle, consent and background-execution policies. Desktop tray/background behavior is not a promise of equivalent mobile behavior.
