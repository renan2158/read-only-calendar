# Contributing

Use the setup and checks in README.md. Keep changes scoped to the read-only first version, and add meaningful regression tests when fixing synchronization, identity, recurrence, encryption or notification behavior.

Provider adapters must implement contract v1 in calendar-providers, use the restricted HTTPS read transport, respect cancellation/limits, return safe errors and preserve prior snapshots on failure. No provider may write calendar events or bypass the encrypted repository/OS credential flow. Never commit credentials, client configuration, personal ICS feeds, vault files, backups, logs with event content, or unsigned update endpoints.

Keep Tauri and React out of the reusable Rust crates. New native capabilities belong in reviewed shell interfaces, not general filesystem/HTTP/SQL permissions for the renderer. For schema/key-envelope changes, document migration, atomicity, recovery and rollback. A passing fixture test must not be described as live-provider certification.

Original contributions are submitted under GPL-3.0-or-later. Preserve third-party notices and verify dependency-license compatibility before adding a dependency.
