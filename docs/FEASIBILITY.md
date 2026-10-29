> Historical prototype report from before implementation. Development was subsequently authorized by the user. See [IMPLEMENTATION.md](IMPLEMENTATION.md) for the current application status; prototype results are not the application test results.

# Privacy-first Calendar — feasibility validation report

Date: 2026-10-29 • Scope: read-only version 1 • Decision: **local feasibility demonstrated; full integration gates remain incomplete. Do not build the full application yet.**

## Outcome

The requirements are consolidated in SPECIFICATION.md. Executable prototypes were created and run for Google protocol behavior, generic ICS consumption, two actual local CalDAV server implementations, and SQLCipher storage through Rust. No full Tauri/React application, real calendar writes, deployed service, or production account connection was created.

Real Google account authorization, a real Proton shared link, externally authenticated/TLS CalDAV, and native desktop security/notifications cannot be certified from these tests. The package supplies read-only live smoke probes and the remaining checklist so these can be completed on the user's device without disclosing credentials in chat.

| Area                  | Evidence achieved                                                                                                                                                                                  | What remains                                                                                                                                        | Gate                                           |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| Google                | Primary documentation reviewed; read-only OAuth authorization URL generated with PKCE/state; simulated initial/paginated/incremental/410/error paths tested                                        | Real desktop OAuth, refresh, revocation, source changes/cancellations, bounded recurrence model, public consent setup                               | **PARTIAL — LIVE ACCOUNT REQUIRED**            |
| Generic CalDAV        | Actual HTTP integration with Radicale and Xandikos on loopback: principal/home/calendar discovery, calendar-query, ETags, updates, deletions, collection sync token                                | Credentialed HTTPS deployment, privileges/auth variants, calendar-multiget, invalid sync-token/fallback cases and Rust adapter                      | **PARTIAL — TWO LOCAL IMPLEMENTATIONS PASSED** |
| Official Proton / ICS | Official sharing constraints reviewed; synthetic ICS parsed and fetched over local HTTP, including validators, changes, simulated revocation, recurrence/DST, moved exceptions, all-day and VALARM | Real paid-account full-view link, source edit/cancellation/refetch, link revocation, actual reminder fields and update delay                        | **PARTIAL — LIVE FEED REQUIRED**               |
| Encrypted storage     | Actual Rust + SQLCipher tests: encrypted data/WAL/FTS, bad-key rejection, authenticated key wrapping, passphrase rewrap, encrypted backup, corruption/integrity, abrupt-process exit               | OS credential-store wrapping, real lifecycle/races/locking, atomic envelope persistence, portable backup UI, cross-platform packaging and hardening | **PARTIAL — RUST LIBRARY PROTOTYPE PASSED**    |
| Notifications         | Requirements specified, including visible time when hiding title                                                                                                                                   | Native notifications/permissions/lock/sleep/background/history on all targets                                                                       | **NOT EXECUTED — PRE-RELEASE GATE**            |

## Executed tests and evidence

### Provider reference probes

**25 tests passed** in the final run. See `evidence/provider-tests.log` and `evidence/provider-tests.xml`. One upstream Radicale deprecation warning concerns `importlib.abc.Traversable` on future Python versions; it did not fail the checks. No credentials or personal events are present in the tests or evidence.

Google tests use synthetic responses; they do not call Google. They validate:

- Authorization request construction with two read-only scopes, state, S256 challenge, and a verifier/challenge match using google-auth-oauthlib.
- Rejection of a substituted OAuth endpoint.
- Initial pagination and final-cursor commit; retention of the previous snapshot when a page fails.
- Incremental query consistency, deletions, and 410-triggered full rebuild.
- Safe failure for 401, 403, 429, 500, and malformed final pages. Backoff scheduling itself is not implemented by the spike.
- Retention of cancelled recurring instances as recurrence exceptions rather than losing their exclusion information.
- Transport-level refusal of calendar write methods, HTTP downgrade, cross-origin endpoint substitution, and credentials embedded in URLs.

The OAuth test checks generation, not browser callback receipt/state rejection or actual Google consent. The production Rust OAuth path and recurrence expansion remain unimplemented.

The CalDAV tests start **Radicale 3.9.0** and **Xandikos 0.4.10** using actual server implementations. Test-only fixture administrators seed and mutate synthetic events. A separate read transport performs discovery and REPORT calls and cannot send event writes. Both tests verify initial ETag/resource content, ETag changes after a source update, collection sync tokens, deletion status in a later delta, and an empty inventory after source deletion. The servers bind only to `127.0.0.1`; fixture authentication is not a production credential-security test. A bounded WSGI input wrapper is used with Xandikos and the standard-library WSGI server.

The ICS tests use a hand-authored synthetic feed, **not an exported Proton feed**. They cover daily recurrence across the New York DST transition, an occurrence moved to a different hour, cancelled-event suppression, an all-day event with exclusive end date, and a ten-minute VALARM. A local HTTP server exercises a conditional GET/304, changed ETag/content, simulated 403 revocation, malformed-feed retention, and redirect refusal. This demonstrates consumer behavior, not Proton server behavior.

### Rust encrypted-storage spike

**18 Rust tests passed**, including one helper invoked by the abrupt-exit test (17 substantive tests plus that helper). See `evidence/rust-storage-tests.log`. `Cargo.lock` is included. The actual compiled SQLCipher/SQLite versions and compile options are in `evidence/rust-storage-build.json`; those are authoritative for this run rather than versions inferred from documentation.

The tests exercise:

1. Encrypted database creation, persistence, closing, and reopening.
2. Rejection of wrong keys and reads without keys.
3. Refusal to open a previously created plaintext SQLite database through the encrypted path.
4. A nonempty live WAL and main database without the known synthetic plaintext canary; encrypted header behavior.
5. Memory temporary-store setting and SQLCipher memory-security setting. The compiled library also reports `TEMP_STORE=2`.
6. SQLCipher encrypted export to a backup using an independent random key; wrong-key rejection and successful restore/read with the backup key.
7. Locked-vault access rejection after connection closure.
8. Argon2id-derived wrapping key, random salt/nonce, AES-256-GCM authentication, serialized envelope roundtrip, wrong-passphrase rejection, and tamper rejection.
9. Passphrase change by rewrapping the same data key and reopening the original database; this tests the cryptographic operation, not atomic envelope-file replacement.
10. Database corruption rejection and a successful SQLCipher integrity check for a valid database.
11. Uncommitted transaction rollback on closure.
12. Abrupt child-process exit without destructors, followed by reopening the encrypted WAL database: the committed event survives, and the uncommitted delete does not.
13. Encrypted FTS5 search-index persistence and a plaintext-canary scan of its database file.

The prototype's key-envelope parameters are fixed to Argon2id v1.3, 64 MiB, three passes, four lanes, and a 32-byte wrapping key. AES-GCM uses a random 96-bit nonce and version-specific authenticated context. These are library-based feasibility choices, not a completed cryptographic audit. Production must validate persisted envelope format, parameter/resource limits, atomic changes/recovery, platform protections, and memory lifetimes. Raw key SQL is generated only from fixed-size binary keys and never logged. Python provider probes hold synthetic or opt-in live source data in process memory; they are not an encrypted production cache.

Expected SQLCipher fixed integrity error messages appear during negative tests. They do not include event data or keys. Absence of a canary is one regression check, not proof that every possible side channel is eliminated.

## Confirmed provider constraints

### Google

Google documents installed-app OAuth through the system browser and a loopback callback, read-only calendar scopes, and incremental synchronization with cursor invalidation. The proposed minimal scope pair is `calendar.calendarlist.readonly` and `calendar.events.readonly`. A publicly distributed app requires the applicable Google project, consent and verification process. The prototype does not prove this app has been verified or granted production access.

References: [desktop OAuth](https://developers.google.com/identity/protocols/oauth2/native-app), [scopes](https://developers.google.com/workspace/calendar/api/auth), [incremental sync](https://developers.google.com/workspace/calendar/api/guides/sync), [events](https://developers.google.com/workspace/calendar/api/v3/reference/events).

### Official Proton sharing

The documented official path matches the read-only scope: generate a sharing URL in Proton Calendar web settings, then subscribe from the app. Creating shared links currently requires a paid Proton plan. Full view exposes event information to link holders; limited view may not provide titles. A downloaded/imported ICS file is a snapshot rather than a refreshing subscription. Proton documents possible update problems with subscribed links, including regeneration as a workaround. The documentation does not establish that every reminder field is available in every shared feed, so the local reminder fallback is necessary.

This does not use Proton's private API or claim that the decrypted shared feed preserves the same encryption boundary as the Proton application. Shared URLs must be treated as secrets.

Reference: [official Proton sharing](https://proton.me/support/share-calendar-via-link).

### SQLCipher

SQLCipher documents encrypted database and journal/WAL page contents and the need to disable disk-backed temporary stores. Its Community Edition uses a BSD-style license. Tests exercised the actual compiled library through Rust rather than substituting application-level encryption of a plain SQLite file.

References: [design](https://www.zetetic.net/sqlcipher/design/), [API](https://www.zetetic.net/sqlcipher/sqlcipher-api/), [license](https://www.zetetic.net/sqlcipher/license/), [Argon2](https://www.rfc-editor.org/rfc/rfc9106).

## Remaining checks before full application development

Run the commands in README.md on the user's device. The optional live probes never write calendar events; prepare and change disposable events in the provider's own interface. Only safe result flags should be shared back.

### Google live account

- [ ] Supply a local Desktop OAuth client configuration and enable the API/test user.
- [ ] Complete system-browser consent; verify only the intended read-only scopes.
- [ ] Read the calendar list and a complete event snapshot.
- [ ] Refresh credentials, then repeat an incremental read.
- [ ] Change/move/cancel synthetic events in Google and verify propagation, including a cancelled recurring occurrence.
- [ ] Revoke consent and verify safe reauthentication behavior without losing cached events.
- [ ] Exercise actual invalid-cursor and interrupted-pagination recovery; the smoke probe does not cover these automatically.
- [ ] Validate selected Rust OAuth/HTTP libraries and recurrence model; the Python reference is not the production implementation.

### CalDAV deployment

- [ ] Run a credentialed HTTPS deployment with valid certificate verification.
- [ ] Read privilege metadata and confirm application-level write refusal even if server credentials allow writes.
- [ ] Validate calendar-multiget, missing/invalid sync-token fallback, and partial propstat failures.
- [ ] Validate required authentication mechanisms and any safe discovery redirects for chosen providers.
- [ ] Port/reference the demonstrated protocol behavior in the Rust connector and run its conformance suite.

### Proton live sharing

- [ ] Generate a full-view link for a disposable calendar from a paid account; keep the link local.
- [ ] Run `python live_probe.py ics` to fetch/parse it, inspect validator availability, and detect VALARM presence without printing contents.
- [ ] Edit/move/cancel test events in Proton and verify later refetches and reminder rescheduling.
- [ ] Test a recurring event, timezone and all-day events using the real feed.
- [ ] Revoke the link and verify a safe failure/stale indicator without logging the URL.
- [ ] Measure refresh behavior and document limits; do not assume the inbound-to-Proton external-calendar polling interval describes outbound shared feeds.

### Storage/platform security

- [ ] On Linux, macOS, and Windows, build the actual SQLCipher configuration and rerun storage checks.
- [ ] Verify actual OS credential-store availability, access/lock failure, and wrapped credential storage without plaintext fallback.
- [ ] Implement and test app lock/OS lock, worker cancellation, in-flight request invalidation, UI cache clearing, notification cancellation and reopening.
- [ ] Test atomic key-envelope changes, recovery, encrypted backup portability, and optional device unlock.
- [ ] Review file permissions, log redaction, memory/swap/crash limitations, library notices, and update-signing design.

## Recommendation

Proceed to completing the blocked live-account and platform feasibility checks, using this package as the reproducible starting point. The evidence supports the proposed architecture, but **does not justify marking Google, Proton, or cross-platform privacy behavior fully validated**. Full application implementation should follow completion of those gates. No secrets need to be sent through chat.
