# Privacy and recovery

Calendar data is stored in SQLCipher-encrypted SQLite, including raw source records, search indexes, synchronization cursors, local preferences, and delivery receipts. No application account, analytics, event relay or automatic crash reporting is used. Provider HTTP requests go directly to configured providers over verified HTTPS. The UI does not load external resources from event descriptions.

## Keys and secrets

A random 256-bit data key encrypts the database. A versioned envelope wraps it using AES-256-GCM and a passphrase-derived Argon2id key (64 MiB, 3 passes, 4 lanes). The envelope includes a random salt and nonce and is atomically replaced when the passphrase changes. The passphrase is not saved. Provider credentials and feed URLs are separately authenticated/encrypted using an HKDF-SHA256 key derived from the vault key before OS credential storage. Linux explicitly uses Secret Service; macOS uses Keychain and Windows uses Credential Manager. Missing storage fails closed; there is no plaintext or mock fallback in application builds.

Optional device unlock places the data key in OS credential storage and reduces independence from the logged-in OS session. Disabling it removes the device entry. Changing a passphrase rewraps the existing key, so it does not invalidate an already enabled device unlock option; disable device unlock separately if desired.

Backups contain a SQLCipher-encrypted export under a new random key, wrapped using the backup passphrase. They do not include provider credentials. No plaintext SQLite intermediary is created. A backup is portable without the original device keychain. Restore requires a fresh vault; reconnect accounts after restore.

## Notifications and locking

Notifications show event title and time unless **Hide event details** is enabled; that setting conceals the title but retains the time. The OS controls lock-screen presentation/history, and delivery is best effort. Do not assume delivered notifications can be erased on all platforms. The app requests removal where supported.

The app pauses event-based sync and alerts while locked. Locking occurs manually, after the configured app-inactivity timeout, or on a detected OS session lock. Linux queries the desktop screen-saver service, Windows checks the input desktop, and macOS checks session-lock information. When detection is unavailable, focus-loss locking is used and the UI displays the limitation. Expected focus changes during explicit Google authorization are exempt from that fallback, but not from actual OS-lock or idle detection.

Closing the window can leave an unlocked app running only if background mode is explicitly enabled. Idle locking still applies. Quitting stops sync and notifications. Missed alerts inside the configured window are summarized on resume/unlock; old alerts are not replayed indefinitely.

## Limits

Local encryption does not make Google-hosted data end-to-end encrypted, hide network metadata, protect an already unlocked device from malware, or guarantee erasure of memory, swap, snapshots, filesystem journals, OS notification history or older backups. Proton sharing exposes the selected shared information to anyone holding the link. TLS and encryption do not change those provider constraints.

There is no passphrase-reset service. Loss of every usable unlocking method makes local data unrecoverable. Provider copies can usually be fetched again; local preferences and notification history require a backup. Removing an account deletes its current OS credential entry and local records; backups and prior OS notifications may retain historical information.

No sensitive event information, credentials, endpoints, SQL values, or raw provider responses are deliberately logged. Production code uses safe error categories. Synthetic negative crypto tests intentionally print fixed SQLCipher integrity diagnostics. Do not enable verbose third-party HTTP/SQL tracing with personal calendars.
