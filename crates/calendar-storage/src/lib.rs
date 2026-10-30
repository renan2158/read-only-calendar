//! Encrypted repository. No database keys or SQL are exposed to the frontend.
mod crypto;
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use calendar_core::{Account, Calendar, Event, Settings};
pub use crypto::{Envelope, random_key, unwrap, wrap};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Unlock failed. Check the passphrase and vault integrity.")]
    Unlock,
    #[error("Encrypted storage could not complete the operation.")]
    Storage,
    #[error("The vault already exists.")]
    Exists,
    #[error("The backup is invalid or its passphrase is incorrect.")]
    Backup,
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub accounts: Vec<Account>,
    pub calendars: Vec<Calendar>,
    pub events: Vec<Event>,
    pub settings: Settings,
}
pub struct Store {
    db: Connection,
    key: Zeroizing<[u8; 32]>,
    directory: PathBuf,
}
#[derive(Serialize, Deserialize)]
struct Backup {
    version: u32,
    envelope: Envelope,
    database: String,
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or(Error::Storage)?;
    let mut f = tempfile::NamedTempFile::new_in(parent).map_err(|_| Error::Storage)?;
    f.write_all(bytes).map_err(|_| Error::Storage)?;
    f.as_file().sync_all().map_err(|_| Error::Storage)?;
    f.persist(path).map_err(|_| Error::Storage)?;
    Ok(())
}
fn connect(path: &Path, key: &[u8; 32]) -> Result<Connection> {
    let db = Connection::open(path).map_err(|_| Error::Storage)?;
    let version: String = db
        .query_row("PRAGMA cipher_version", [], |r| r.get(0))
        .map_err(|_| Error::Storage)?;
    if version.is_empty() {
        return Err(Error::Storage);
    }
    let sql = Zeroizing::new(format!("PRAGMA key=\"x'{}'\";", hex::encode(key)));
    db.execute_batch(sql.as_str()).map_err(|_| Error::Unlock)?;
    db.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|_| Error::Unlock)?;
    db.execute_batch("PRAGMA temp_store=MEMORY; PRAGMA cipher_memory_security=ON; PRAGMA secure_delete=ON; PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS state(id INTEGER PRIMARY KEY CHECK(id=1),body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS receipts(id TEXT PRIMARY KEY,delivered INTEGER NOT NULL); CREATE VIRTUAL TABLE IF NOT EXISTS event_search USING fts5(id UNINDEXED,title,description);").map_err(|_|Error::Storage)?;
    Ok(db)
}
impl Store {
    pub fn exists(dir: &Path) -> bool {
        dir.join("vault.keywrap").exists()
    }
    pub fn create(dir: &Path, passphrase: &[u8]) -> Result<Self> {
        if Self::exists(dir) || dir.join("calendar.db").exists() {
            return Err(Error::Exists);
        }
        if passphrase.len() < 12 {
            return Err(Error::Unlock);
        }
        fs::create_dir_all(dir).map_err(|_| Error::Storage)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                .map_err(|_| Error::Storage)?;
        }
        let key = random_key().map_err(|_| Error::Storage)?;
        let envelope = wrap(passphrase, &key).map_err(|_| Error::Storage)?;
        let db = connect(&dir.join("calendar.db"), &key)?;
        let mut store = Self {
            db,
            key,
            directory: dir.into(),
        };
        store.save(&Snapshot::default())?;
        atomic(
            &dir.join("vault.keywrap"),
            &serde_json::to_vec(&envelope).map_err(|_| Error::Storage)?,
        )?;
        Ok(store)
    }
    pub fn open(dir: &Path, passphrase: &[u8]) -> Result<Self> {
        let bytes = fs::read(dir.join("vault.keywrap")).map_err(|_| Error::Unlock)?;
        if bytes.len() > 4096 {
            return Err(Error::Unlock);
        }
        let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|_| Error::Unlock)?;
        let key = unwrap(passphrase, &envelope).map_err(|_| Error::Unlock)?;
        Self::open_key(dir, key)
    }
    pub fn open_key(dir: &Path, key: Zeroizing<[u8; 32]>) -> Result<Self> {
        if !dir.join("calendar.db").is_file() {
            return Err(Error::Unlock);
        }
        let db = connect(&dir.join("calendar.db"), &key)?;
        Ok(Self {
            db,
            key,
            directory: dir.into(),
        })
    }
    pub fn device_key(&self) -> &[u8; 32] {
        &self.key
    }
    pub fn load(&self) -> Result<Snapshot> {
        let body: String = self
            .db
            .query_row("SELECT body FROM state WHERE id=1", [], |r| r.get(0))
            .map_err(|_| Error::Storage)?;
        serde_json::from_str(&body).map_err(|_| Error::Storage)
    }
    pub fn save(&mut self, s: &Snapshot) -> Result<()> {
        let body = Zeroizing::new(serde_json::to_string(s).map_err(|_| Error::Storage)?);
        let tx = self.db.transaction().map_err(|_| Error::Storage)?;
        tx.execute(
            "INSERT INTO state VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET body=excluded.body",
            params![body.as_str()],
        )
        .map_err(|_| Error::Storage)?;
        tx.execute("DELETE FROM event_search", [])
            .map_err(|_| Error::Storage)?;
        for e in &s.events {
            tx.execute(
                "INSERT INTO event_search VALUES(?1,?2,?3)",
                params![e.id, e.title, e.description],
            )
            .map_err(|_| Error::Storage)?;
        }
        tx.commit().map_err(|_| Error::Storage)
    }
    pub fn delivered(&self, id: &str) -> bool {
        self.db
            .query_row("SELECT 1 FROM receipts WHERE id=?1", params![id], |r| {
                r.get::<_, i32>(0)
            })
            .is_ok()
    }
    pub fn receipt(&self, id: &str, now: i64) -> Result<()> {
        self.db
            .execute(
                "INSERT OR IGNORE INTO receipts VALUES(?1,?2)",
                params![id, now],
            )
            .map_err(|_| Error::Storage)?;
        Ok(())
    }
    pub fn change_passphrase(&self, current: &[u8], next: &[u8]) -> Result<()> {
        if next.len() < 12 {
            return Err(Error::Unlock);
        }
        let check = Self::open(&self.directory, current)?;
        drop(check);
        let e = wrap(next, &self.key).map_err(|_| Error::Storage)?;
        atomic(
            &self.directory.join("vault.keywrap"),
            &serde_json::to_vec(&e).map_err(|_| Error::Storage)?,
        )
    }
    pub fn seal_secret(&self, id: &str, bytes: &[u8]) -> Result<String> {
        let mut derived = Zeroizing::new([0u8; 32]);
        hkdf::Hkdf::<Sha256>::new(Some(b"read-only-calendar-v1"), self.key.as_ref())
            .expand(b"credential-wrap-v1", derived.as_mut())
            .map_err(|_| Error::Storage)?;
        let c = Aes256Gcm::new_from_slice(derived.as_ref()).map_err(|_| Error::Storage)?;
        let mut n = [0; 12];
        getrandom::fill(&mut n).map_err(|_| Error::Storage)?;
        let data = c
            .encrypt(
                Nonce::from_slice(&n),
                Payload {
                    msg: bytes,
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| Error::Storage)?;
        Ok(format!("{}:{}", hex::encode(n), hex::encode(data)))
    }
    pub fn open_secret(&self, id: &str, encoded: &str) -> Result<Zeroizing<Vec<u8>>> {
        let (n, data) = encoded.split_once(':').ok_or(Error::Storage)?;
        let n = hex::decode(n).map_err(|_| Error::Storage)?;
        if n.len() != 12 {
            return Err(Error::Storage);
        }
        let mut derived = Zeroizing::new([0u8; 32]);
        hkdf::Hkdf::<Sha256>::new(Some(b"read-only-calendar-v1"), self.key.as_ref())
            .expand(b"credential-wrap-v1", derived.as_mut())
            .map_err(|_| Error::Storage)?;
        let c = Aes256Gcm::new_from_slice(derived.as_ref()).map_err(|_| Error::Storage)?;
        let data = hex::decode(data).map_err(|_| Error::Storage)?;
        Ok(Zeroizing::new(
            c.decrypt(
                Nonce::from_slice(&n),
                Payload {
                    msg: &data,
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| Error::Storage)?,
        ))
    }
    pub fn backup(&self, path: &Path, passphrase: &[u8]) -> Result<()> {
        if passphrase.len() < 12 {
            return Err(Error::Backup);
        }
        let temp = tempfile::tempdir().map_err(|_| Error::Storage)?;
        let file = temp.path().join("encrypted.db");
        let key = random_key().map_err(|_| Error::Storage)?;
        let sql = Zeroizing::new(format!(
            "ATTACH DATABASE ?1 AS backup KEY \"x'{}'\"",
            hex::encode(key.as_ref())
        ));
        self.db
            .execute(sql.as_str(), params![file.to_string_lossy().as_ref()])
            .map_err(|_| Error::Storage)?;
        let exported = self.db.execute_batch("SELECT sqlcipher_export('backup');");
        let detached = self.db.execute_batch("DETACH DATABASE backup;");
        exported.map_err(|_| Error::Storage)?;
        detached.map_err(|_| Error::Storage)?;
        let b = Backup {
            version: 1,
            envelope: wrap(passphrase, &key).map_err(|_| Error::Backup)?,
            database: hex::encode(fs::read(file).map_err(|_| Error::Storage)?),
        };
        atomic(path, &serde_json::to_vec(&b).map_err(|_| Error::Storage)?)
    }
    pub fn restore(
        dir: &Path,
        path: &Path,
        passphrase: &[u8],
        new_passphrase: &[u8],
    ) -> Result<Self> {
        if Self::exists(dir) || dir.join("calendar.db").exists() {
            return Err(Error::Exists);
        }
        if fs::metadata(path).map_err(|_| Error::Backup)?.len() > 128 * 1024 * 1024 {
            return Err(Error::Backup);
        }
        let b: Backup = serde_json::from_slice(&fs::read(path).map_err(|_| Error::Backup)?)
            .map_err(|_| Error::Backup)?;
        if b.version != 1 {
            return Err(Error::Backup);
        }
        let key = unwrap(passphrase, &b.envelope).map_err(|_| Error::Backup)?;
        let temp = tempfile::tempdir().map_err(|_| Error::Storage)?;
        let file = temp.path().join("backup.db");
        fs::write(&file, hex::decode(b.database).map_err(|_| Error::Backup)?)
            .map_err(|_| Error::Backup)?;
        let db = connect(&file, &key)?;
        let data: String = db
            .query_row("SELECT body FROM state WHERE id=1", [], |r| r.get(0))
            .map_err(|_| Error::Backup)?;
        let snapshot: Snapshot = serde_json::from_str(&data).map_err(|_| Error::Backup)?;
        let mut store = Self::create(dir, new_passphrase)?;
        store.save(&snapshot)?;
        Ok(store)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repository_roundtrip_and_wrapped_secrets() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::create(d.path(), b"a sufficiently long passphrase").unwrap();
        let wrapped = s.seal_secret("account", b"synthetic token").unwrap();
        assert!(!wrapped.contains("synthetic"));
        assert_eq!(
            s.open_secret("account", &wrapped).unwrap().as_slice(),
            b"synthetic token"
        );
        assert!(s.open_secret("other", &wrapped).is_err());
        drop(s);
        assert!(Store::open(d.path(), b"bad").is_err());
        assert!(
            Store::open(d.path(), b"a sufficiently long passphrase")
                .unwrap()
                .load()
                .unwrap()
                .accounts
                .is_empty()
        );
    }
    #[test]
    fn backup_and_passphrase_change() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::create(&d.path().join("a"), b"original passphrase").unwrap();
        s.change_passphrase(b"original passphrase", b"replacement passphrase")
            .unwrap();
        let p = d.path().join("backup.rcvault");
        s.backup(&p, b"backup passphrase").unwrap();
        let restored = Store::restore(
            &d.path().join("b"),
            &p,
            b"backup passphrase",
            b"restored passphrase",
        )
        .unwrap();
        assert!(restored.load().is_ok());
        assert!(Store::open(&d.path().join("a"), b"original passphrase").is_err());
    }
    #[test]
    fn receipts_survive_restart() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::create(d.path(), b"another long passphrase").unwrap();
        s.receipt("opaque", 10).unwrap();
        drop(s);
        let s = Store::open(d.path(), b"another long passphrase").unwrap();
        assert!(s.delivered("opaque"));
    }
}
