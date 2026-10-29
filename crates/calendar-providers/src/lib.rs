//! Version 1 read-only provider contract. No adapter can submit calendar writes.
use calendar_core::{Calendar, Event};
use reqwest::Method;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use thiserror::Error;
use url::Url;
pub mod caldav;
pub mod google;
pub mod ics;

#[derive(Debug, Error)]
pub enum Error {
    #[error(
        "The provider could not be reached securely. Check the connection and calendar settings."
    )]
    Network,
    #[error(
        "The provider rejected access. Reconnect the account or replace its subscription link."
    )]
    Authentication,
    #[error("The provider is temporarily rate-limited. Synchronization will retry later.")]
    RateLimit,
    #[error(
        "The provider returned invalid or unsupported calendar data. Cached events were retained."
    )]
    Invalid,
    #[error("Synchronization was cancelled.")]
    Cancelled,
    #[error("The calendar exceeds the safety limits.")]
    Limit,
    #[error("Google authorization failed or timed out. Try connecting again.")]
    OAuth,
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Default)]
pub struct Cancellation(pub Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst)
    }
    pub fn check(&self) -> Result<()> {
        if self.0.load(Ordering::SeqCst) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Serialize, Deserialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
#[serde(tag = "provider", rename_all = "camelCase")]
pub enum Credentials {
    Ics {
        url: String,
    },
    Proton {
        url: String,
    },
    Caldav {
        url: String,
        username: String,
        password: String,
    },
    Google {
        client_id: String,
        client_secret: String,
        refresh_token: String,
    },
}
impl Credentials {
    pub fn provider(&self) -> &'static str {
        match self {
            Self::Ics { .. } => "ics",
            Self::Proton { .. } => "proton",
            Self::Caldav { .. } => "caldav",
            Self::Google { .. } => "google",
        }
    }
}
pub struct SyncedCalendar {
    pub calendar: Calendar,
    pub events: Vec<Event>,
}
pub struct SyncResult {
    pub calendars: Vec<SyncedCalendar>,
}
pub struct Context<'a> {
    pub account_id: &'a str,
    pub calendars: &'a [Calendar],
    pub cached: &'a [Event],
    pub timezone: &'a str,
    pub cancellation: Cancellation,
}
pub trait CalendarProvider {
    fn contract_version(&self) -> u32 {
        1
    }
    fn sync(&self, context: &Context<'_>, credentials: &Credentials) -> Result<SyncResult>;
}
pub fn sync(context: &Context<'_>, credentials: &Credentials) -> Result<SyncResult> {
    match credentials {
        Credentials::Ics { .. } | Credentials::Proton { .. } => ics::Ics.sync(context, credentials),
        Credentials::Caldav { .. } => caldav::CalDav.sync(context, credentials),
        Credentials::Google { .. } => google::Google.sync(context, credentials),
    }
}
pub struct ReadClient {
    client: Client,
    cancel: Cancellation,
}
pub struct Response {
    pub status: u16,
    pub etag: Option<String>,
    pub modified: Option<String>,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Copy)]
pub enum ReadMethod {
    Get,
    Propfind,
    Report,
}
impl ReadClient {
    pub fn new(cancel: Cancellation) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .user_agent("ReadOnlyCalendar/0.1")
            .build()
            .map_err(|_| Error::Network)?;
        Ok(Self { client, cancel })
    }
    pub fn endpoint(url: &str) -> Result<Url> {
        let u = Url::parse(url).map_err(|_| Error::Invalid)?;
        if u.scheme() != "https"
            || !u.username().is_empty()
            || u.password().is_some()
            || u.fragment().is_some()
        {
            return Err(Error::Invalid);
        }
        Ok(u)
    }
    pub fn request(
        &self,
        method: ReadMethod,
        url: &str,
        auth: Option<(&str, &str)>,
        bearer: Option<&str>,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Result<Response> {
        self.cancel.check()?;
        let u = Self::endpoint(url)?;
        let method = match method {
            ReadMethod::Get => Method::GET,
            ReadMethod::Propfind => Method::from_bytes(b"PROPFIND").map_err(|_| Error::Invalid)?,
            ReadMethod::Report => Method::from_bytes(b"REPORT").map_err(|_| Error::Invalid)?,
        };
        let mut request = self.client.request(method, u);
        if let Some((u, p)) = auth {
            request = request.basic_auth(u, Some(p));
        }
        if let Some(token) = bearer {
            request = request.bearer_auth(token);
        }
        for (k, v) in headers {
            request = request.header(*k, *v);
        }
        if let Some(b) = body {
            request = request.body(b.to_owned());
        }
        let request = request.build().map_err(|_| Error::Network)?;
        let mut attempt = 0u32;
        let mut r = loop {
            self.cancel.check()?;
            let response = self
                .client
                .execute(request.try_clone().ok_or(Error::Network)?)
                .map_err(|_| Error::Network)?;
            let code = response.status().as_u16();
            if (code == 429 || code >= 500) && attempt < 2 {
                let delay = response
                    .headers()
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(1 << attempt)
                    .min(30);
                drop(response);
                for _ in 0..delay * 10 {
                    self.cancel.check()?;
                    std::thread::sleep(Duration::from_millis(100));
                }
                attempt += 1;
                continue;
            }
            break response;
        };
        let status = r.status().as_u16();
        if status == 401 || status == 403 {
            return Err(Error::Authentication);
        }
        if status == 429 {
            return Err(Error::RateLimit);
        }
        if (300..400).contains(&status) && status != 304 {
            return Err(Error::Network);
        }
        let modified = r
            .headers()
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let etag = r
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        if r.content_length()
            .is_some_and(|n| n > calendar_core::MAX_FEED_BYTES as u64)
        {
            return Err(Error::Limit);
        }
        use std::io::Read;
        let mut bytes = Vec::new();
        r.by_ref()
            .take((calendar_core::MAX_FEED_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Network)?;
        self.cancel.check()?;
        if bytes.len() > calendar_core::MAX_FEED_BYTES {
            return Err(Error::Limit);
        }
        Ok(Response {
            status,
            etag,
            modified,
            bytes,
        })
    }
}
pub fn calendar(context: &Context<'_>, source: &str, name: &str) -> Calendar {
    let id = calendar_core::identity(&[context.account_id, source]);
    if let Some(c) = context.calendars.iter().find(|c| c.id == id) {
        return c.clone();
    }
    Calendar {
        id,
        account_id: context.account_id.into(),
        source_id: source.into(),
        name: name.into(),
        color: "#668b80".into(),
        visible: true,
        last_sync: None,
        error: None,
        cursor: None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_insecure_endpoints() {
        for u in [
            "http://example.com",
            "https://user:secret@example.com/",
            "https://example.com/#secret",
        ] {
            assert!(ReadClient::endpoint(u).is_err());
        }
    }
    #[test]
    fn cancellation_is_shared() {
        let c = Cancellation::default();
        let clone = c.clone();
        c.cancel();
        assert!(clone.check().is_err());
    }
}
