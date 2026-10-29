//! Vault key-envelope primitives and isolated encrypted-storage regression tests.
//! SPDX-License-Identifier: GPL-3.0-or-later
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use argon2::{Algorithm, Argon2, Params, Version};
#[cfg(test)]
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::path::Path;
use zeroize::Zeroizing;

const AAD: &[u8] = b"calendar-storage-spike:key-envelope:v1";
#[cfg(test)]
const CANARY: &str = "SYNTHETIC_PRIVATE_CALENDAR_CANARY_8472";

#[derive(Debug)]
pub enum Error {
    Random,
    Crypto,
    Envelope,
    Locked,
    Database,
}
impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::Database
    }
}
pub type Result<T> = std::result::Result<T, Error>;

pub fn random_key() -> Result<Zeroizing<[u8; 32]>> {
    let mut key = Zeroizing::new([0; 32]);
    getrandom::fill(key.as_mut()).map_err(|_| Error::Random)?;
    Ok(key)
}

#[derive(Serialize, Deserialize)]
pub struct Envelope {
    version: u8,
    salt: [u8; 16],
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}
fn derive(passphrase: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(65536, 3, 4, Some(32)).map_err(|_| Error::Crypto)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0; 32]);
    argon
        .hash_password_into(passphrase, salt, key.as_mut())
        .map_err(|_| Error::Crypto)?;
    Ok(key)
}
pub fn wrap(passphrase: &[u8], key: &[u8; 32]) -> Result<Envelope> {
    let mut salt = [0; 16];
    let mut nonce = [0; 12];
    getrandom::fill(&mut salt).map_err(|_| Error::Random)?;
    getrandom::fill(&mut nonce).map_err(|_| Error::Random)?;
    let kek = derive(passphrase, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(kek.as_ref()).map_err(|_| Error::Crypto)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: key, aad: AAD })
        .map_err(|_| Error::Crypto)?;
    Ok(Envelope {
        version: 1,
        salt,
        nonce,
        ciphertext,
    })
}
pub fn unwrap(passphrase: &[u8], envelope: &Envelope) -> Result<Zeroizing<[u8; 32]>> {
    if envelope.version != 1 || envelope.ciphertext.len() != 48 {
        return Err(Error::Envelope);
    }
    let kek = derive(passphrase, &envelope.salt)?;
    let cipher = Aes256Gcm::new_from_slice(kek.as_ref()).map_err(|_| Error::Crypto)?;
    let bytes = Zeroizing::new(
        cipher
            .decrypt(
                Nonce::from_slice(&envelope.nonce),
                Payload {
                    msg: &envelope.ciphertext,
                    aad: AAD,
                },
            )
            .map_err(|_| Error::Crypto)?,
    );
    let mut key = Zeroizing::new([0; 32]);
    key.copy_from_slice(&bytes);
    Ok(key)
}
#[cfg(test)]
fn keyed_connection(path: &Path, key: &[u8; 32]) -> Result<Connection> {
    let c = Connection::open(path)?;
    let version: String = c.query_row("PRAGMA cipher_version", [], |r| r.get(0))?;
    if version.is_empty() {
        return Err(Error::Database);
    }
    // Hex comes only from a generated fixed-size key; never user SQL or logging.
    let secret_sql = Zeroizing::new(format!("PRAGMA key=\"x'{}'\"", hex::encode(key)));
    c.execute_batch(secret_sql.as_str())?;
    c.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })?;
    c.execute_batch(
        "PRAGMA temp_store=MEMORY; PRAGMA cipher_memory_security=ON; PRAGMA secure_delete=ON;",
    )?;
    Ok(c)
}
#[cfg(test)]
pub struct Vault {
    connection: Option<Connection>,
}
#[cfg(test)]
impl Vault {
    pub fn open(path: &Path, key: &[u8; 32]) -> Result<Self> {
        let c = keyed_connection(path, key)?;
        c.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS events(id TEXT PRIMARY KEY,title TEXT NOT NULL);")?;
        Ok(Self {
            connection: Some(c),
        })
    }
    fn connection(&self) -> Result<&Connection> {
        self.connection.as_ref().ok_or(Error::Locked)
    }
    pub fn put_synthetic_event(&self) -> Result<()> {
        self.connection()?.execute(
            "INSERT OR REPLACE INTO events VALUES(?1,?2)",
            params!["synthetic-id", CANARY],
        )?;
        Ok(())
    }
    pub fn count(&self) -> Result<i64> {
        Ok(self
            .connection()?
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))?)
    }
    pub fn lock(&mut self) -> Result<()> {
        if let Some(c) = self.connection.take() {
            c.close().map_err(|_| Error::Database)?;
        }
        Ok(())
    }
    pub fn encrypted_backup(&self, path: &Path, key: &[u8; 32]) -> Result<()> {
        let c = self.connection()?;
        let sql = Zeroizing::new(format!(
            "ATTACH DATABASE ?1 AS backup KEY \"x'{}'\"",
            hex::encode(key)
        ));
        c.execute(sql.as_str(), params![path.to_str().ok_or(Error::Database)?])?;
        let result = c.execute_batch("SELECT sqlcipher_export('backup');");
        c.execute_batch("DETACH DATABASE backup;")?;
        result?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn setup() -> (tempfile::TempDir, Zeroizing<[u8; 32]>, Vault) {
        let d = tempfile::tempdir().unwrap();
        let k = random_key().unwrap();
        let v = Vault::open(&d.path().join("vault.db"), &k).unwrap();
        (d, k, v)
    }
    fn contains(bytes: &[u8], needle: &[u8]) -> bool {
        bytes.windows(needle.len()).any(|w| w == needle)
    }
    #[test]
    fn encrypted_reopen() {
        let (d, k, mut v) = setup();
        v.put_synthetic_event().unwrap();
        v.lock().unwrap();
        assert_eq!(
            Vault::open(&d.path().join("vault.db"), &k)
                .unwrap()
                .count()
                .unwrap(),
            1
        );
    }
    #[test]
    fn wrong_key_rejected() {
        let (d, _k, mut v) = setup();
        v.put_synthetic_event().unwrap();
        v.lock().unwrap();
        assert!(Vault::open(&d.path().join("vault.db"), &random_key().unwrap()).is_err());
    }
    #[test]
    fn no_key_rejected() {
        let (d, _k, mut v) = setup();
        v.put_synthetic_event().unwrap();
        v.lock().unwrap();
        let c = Connection::open(d.path().join("vault.db")).unwrap();
        assert!(
            c.query_row("SELECT count(*) FROM sqlite_master", [], |r| r
                .get::<_, i64>(0))
                .is_err()
        );
    }
    #[test]
    fn plaintext_database_rejected() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("plain.db");
        let c = Connection::open(&p).unwrap();
        c.execute_batch("CREATE TABLE secret(x);").unwrap();
        drop(c);
        assert!(Vault::open(&p, &random_key().unwrap()).is_err());
    }
    #[test]
    fn wal_and_database_hide_canary() {
        let (d, _k, v) = setup();
        v.put_synthetic_event().unwrap();
        let wal = d.path().join("vault.db-wal");
        assert!(wal.exists());
        assert!(fs::metadata(&wal).unwrap().len() > 32);
        for p in [d.path().join("vault.db"), wal] {
            let b = fs::read(p).unwrap();
            assert!(!contains(&b, CANARY.as_bytes()));
            assert!(!b.starts_with(b"SQLite format 3"));
        }
    }
    #[test]
    fn temporary_store_and_memory_security() {
        let (_d, _k, v) = setup();
        let c = v.connection().unwrap();
        assert_eq!(
            c.query_row("PRAGMA temp_store", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            c.query_row("PRAGMA cipher_memory_security", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "1"
        );
    }
    #[test]
    fn encrypted_backup_restores_with_independent_key() {
        let (d, k, v) = setup();
        v.put_synthetic_event().unwrap();
        let bkey = random_key().unwrap();
        let p = d.path().join("backup.db");
        v.encrypted_backup(&p, &bkey).unwrap();
        let b = fs::read(&p).unwrap();
        assert!(!contains(&b, CANARY.as_bytes()));
        assert!(!b.starts_with(b"SQLite format 3"));
        assert!(Vault::open(&p, &k).is_err());
        assert_eq!(Vault::open(&p, &bkey).unwrap().count().unwrap(), 1);
    }
    #[test]
    fn locked_vault_rejects_access() {
        let (_d, _k, mut v) = setup();
        v.lock().unwrap();
        assert!(matches!(v.count(), Err(Error::Locked)));
        assert!(matches!(v.put_synthetic_event(), Err(Error::Locked)));
    }
    #[test]
    fn envelope_roundtrip_and_serialization() {
        let k = random_key().unwrap();
        let e = wrap(b"synthetic-long-passphrase", &k).unwrap();
        let json = serde_json::to_vec(&e).unwrap();
        assert!(!contains(&json, b"synthetic-long-passphrase"));
        assert!(!contains(&json, hex::encode(k.as_ref()).as_bytes()));
        let e: Envelope = serde_json::from_slice(&json).unwrap();
        assert_eq!(
            unwrap(b"synthetic-long-passphrase", &e).unwrap().as_ref(),
            k.as_ref()
        );
    }
    #[test]
    fn wrong_passphrase_rejected() {
        let k = random_key().unwrap();
        let e = wrap(b"correct synthetic passphrase", &k).unwrap();
        assert!(unwrap(b"wrong passphrase", &e).is_err());
    }
    #[test]
    fn changed_envelope_rejected() {
        let k = random_key().unwrap();
        let mut e = wrap(b"synthetic passphrase", &k).unwrap();
        e.ciphertext[0] ^= 1;
        assert!(unwrap(b"synthetic passphrase", &e).is_err());
        e.version = 99;
        assert!(matches!(
            unwrap(b"synthetic passphrase", &e),
            Err(Error::Envelope)
        ));
    }
    #[test]
    fn passphrase_change_preserves_database() {
        let (d, k, mut v) = setup();
        v.put_synthetic_event().unwrap();
        let first = wrap(b"old synthetic phrase", &k).unwrap();
        let unwrapped = unwrap(b"old synthetic phrase", &first).unwrap();
        let second = wrap(b"new synthetic phrase", &unwrapped).unwrap();
        v.lock().unwrap();
        assert!(unwrap(b"old synthetic phrase", &second).is_err());
        let changed = unwrap(b"new synthetic phrase", &second).unwrap();
        assert_eq!(
            Vault::open(&d.path().join("vault.db"), &changed)
                .unwrap()
                .count()
                .unwrap(),
            1
        );
    }
    #[test]
    fn corruption_is_detected() {
        let (d, k, mut v) = setup();
        v.put_synthetic_event().unwrap();
        v.lock().unwrap();
        let p = d.path().join("vault.db");
        let mut b = fs::read(&p).unwrap();
        b[100] ^= 0xff;
        fs::write(&p, b).unwrap();
        assert!(Vault::open(&p, &k).is_err());
    }
    #[test]
    fn uncommitted_changes_do_not_survive_reopen() {
        let (d, k, mut v) = setup();
        v.put_synthetic_event().unwrap();
        v.connection()
            .unwrap()
            .execute_batch("BEGIN; DELETE FROM events;")
            .unwrap();
        v.lock().unwrap();
        assert_eq!(
            Vault::open(&d.path().join("vault.db"), &k)
                .unwrap()
                .count()
                .unwrap(),
            1
        );
    }
    #[test]
    fn encrypted_search_index() {
        let (d, k, mut v) = setup();
        v.connection()
            .unwrap()
            .execute_batch("CREATE VIRTUAL TABLE event_search USING fts5(title);")
            .unwrap();
        v.connection()
            .unwrap()
            .execute(
                "INSERT INTO event_search(title) VALUES(?1)",
                params![CANARY],
            )
            .unwrap();
        assert_eq!(
            v.connection()
                .unwrap()
                .query_row(
                    "SELECT count(*) FROM event_search WHERE event_search MATCH ?1",
                    params![CANARY],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        v.lock().unwrap();
        let bytes = fs::read(d.path().join("vault.db")).unwrap();
        assert!(!contains(&bytes, CANARY.as_bytes()));
        assert_eq!(
            Vault::open(&d.path().join("vault.db"), &k)
                .unwrap()
                .connection()
                .unwrap()
                .query_row("SELECT count(*) FROM event_search", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn crash_child_helper() {
        if let Ok(path) = std::env::var("CALENDAR_SPIKE_CRASH_PATH") {
            use std::io::Read;
            let mut key = Zeroizing::new([0u8; 32]);
            std::io::stdin().read_exact(key.as_mut()).unwrap();
            let v = Vault::open(Path::new(&path), &key).unwrap();
            v.put_synthetic_event().unwrap();
            v.connection()
                .unwrap()
                .execute_batch("BEGIN; DELETE FROM events;")
                .unwrap();
            std::process::exit(23); // No destructors: simulate abrupt process termination.
        }
    }
    #[test]
    fn abrupt_process_exit_preserves_committed_wal() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let d = tempfile::tempdir().unwrap();
        let k = random_key().unwrap();
        let path = d.path().join("crash.db");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "crypto::tests::crash_child_helper",
                "--nocapture",
            ])
            .env("CALENDAR_SPIKE_CRASH_PATH", &path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(k.as_ref()).unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(23));
        assert_eq!(Vault::open(&path, &k).unwrap().count().unwrap(), 1);
    }

    #[test]
    fn cipher_integrity_check_succeeds() {
        let (_d, _k, v) = setup();
        v.put_synthetic_event().unwrap();
        let errors: Vec<String> = v
            .connection()
            .unwrap()
            .prepare("PRAGMA cipher_integrity_check")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert!(errors.is_empty());
    }
}
