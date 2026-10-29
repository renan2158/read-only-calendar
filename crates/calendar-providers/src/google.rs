use super::*;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    time::Instant,
};
pub const SCOPES: &str = "https://www.googleapis.com/auth/calendar.calendarlist.readonly https://www.googleapis.com/auth/calendar.events.readonly";
pub struct Google;
fn oauth_client() -> Result<Client> {
    Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| Error::OAuth)
}
fn token(parameters: &[(&str, &str)]) -> Result<Value> {
    let r = oauth_client()?
        .post("https://oauth2.googleapis.com/token")
        .form(parameters)
        .send()
        .map_err(|_| Error::OAuth)?;
    if !r.status().is_success() {
        return Err(Error::Authentication);
    }
    let v: Value = r.json().map_err(|_| Error::OAuth)?;
    Ok(v)
}
pub fn authorize(
    client_id: &str,
    client_secret: &str,
    cancel: Cancellation,
    open: impl FnOnce(&str) -> Result<()>,
) -> Result<Credentials> {
    if !client_id.ends_with(".apps.googleusercontent.com") {
        return Err(Error::OAuth);
    }
    let mut random = [0u8; 64];
    getrandom::fill(&mut random).map_err(|_| Error::OAuth)?;
    let verifier = URL_SAFE_NO_PAD.encode(&random[..32]);
    let state = URL_SAFE_NO_PAD.encode(&random[32..]);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| Error::OAuth)?;
    listener.set_nonblocking(true).map_err(|_| Error::OAuth)?;
    let redirect = format!(
        "http://127.0.0.1:{}/",
        listener.local_addr().map_err(|_| Error::OAuth)?.port()
    );
    let mut url =
        Url::parse("https://accounts.google.com/o/oauth2/v2/auth").map_err(|_| Error::OAuth)?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", client_id),
        ("redirect_uri", redirect.as_str()),
        ("response_type", "code"),
        ("scope", SCOPES),
        ("state", state.as_str()),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ]);
    open(url.as_str())?;
    let started = Instant::now();
    loop {
        cancel.check()?;
        if started.elapsed() > Duration::from_secs(180) {
            return Err(Error::OAuth);
        }
        if let Ok((mut socket, _)) = listener.accept() {
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .map_err(|_| Error::OAuth)?;
            let mut bytes = [0; 8192];
            let n = socket.read(&mut bytes).map_err(|_| Error::OAuth)?;
            let line = std::str::from_utf8(&bytes[..n])
                .map_err(|_| Error::OAuth)?
                .lines()
                .next()
                .ok_or(Error::OAuth)?;
            let path = line.split_whitespace().nth(1).ok_or(Error::OAuth)?;
            let callback = Url::parse(&format!("{redirect}{}", path.trim_start_matches('/')))
                .map_err(|_| Error::OAuth)?;
            let pairs: BTreeMap<_, _> = callback.query_pairs().into_owned().collect();
            let valid = pairs.get("state") == Some(&state) && pairs.contains_key("code");
            let body = if valid {
                "Authorization complete. Return to Read-only Calendar."
            } else {
                "Authorization was not accepted. You may close this page."
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes());
            if !valid {
                continue;
            }
            let code = pairs.get("code").ok_or(Error::OAuth)?;
            let t = token(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("redirect_uri", &redirect),
                ("code_verifier", &verifier),
            ])?;
            return Ok(Credentials::Google {
                client_id: client_id.into(),
                client_secret: client_secret.into(),
                refresh_token: t["refresh_token"].as_str().ok_or(Error::OAuth)?.into(),
            });
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn fetch(
    client: &ReadClient,
    path: &str,
    query: &[(&str, String)],
    access: &str,
) -> Result<Response> {
    let mut url = Url::parse("https://www.googleapis.com/calendar/v3/")
        .map_err(|_| Error::Invalid)?
        .join(path)
        .map_err(|_| Error::Invalid)?;
    url.query_pairs_mut()
        .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
    client.request(ReadMethod::Get, url.as_str(), None, Some(access), &[], None)
}
fn pages(
    client: &ReadClient,
    path: &str,
    mut query: Vec<(&str, String)>,
    access: &str,
) -> Result<(Vec<Value>, Option<String>, bool)> {
    let mut items = Vec::new();
    for _ in 0..100 {
        let r = fetch(client, path, &query, access)?;
        if r.status == 410 {
            return Ok((vec![], None, true));
        }
        if r.status != 200 {
            return Err(Error::Network);
        }
        let v: Value = serde_json::from_slice(&r.bytes).map_err(|_| Error::Invalid)?;
        if let Some(array) = v.get("items") {
            items.extend(array.as_array().ok_or(Error::Invalid)?.clone());
        }
        if items.len() > calendar_core::MAX_EVENTS {
            return Err(Error::Limit);
        }
        if let Some(p) = v["nextPageToken"].as_str() {
            query.retain(|(k, _)| *k != "pageToken");
            query.push(("pageToken", p.into()));
        } else {
            return Ok((items, v["nextSyncToken"].as_str().map(str::to_owned), false));
        }
    }
    Err(Error::Limit)
}
fn google_time(v: &Value, default_zone: &str) -> Result<(i64, bool, String, String)> {
    let zone = v["timeZone"].as_str().unwrap_or(default_zone).to_owned();
    if let Some(s) = v["date"].as_str() {
        let basic = s.replace('-', "");
        let timestamp = calendar_core::parse_time(&basic, &zone).map_err(|_| Error::Invalid)?;
        return Ok((
            timestamp,
            true,
            zone.clone(),
            format!("DTSTART;TZID={zone}:{basic}T000000"),
        ));
    }
    let date = DateTime::parse_from_rfc3339(v["dateTime"].as_str().ok_or(Error::Invalid)?)
        .map_err(|_| Error::Invalid)?;
    let naive = if let Ok(tz) = zone.parse::<chrono_tz::Tz>() {
        date.with_timezone(&tz).format("%Y%m%dT%H%M%S").to_string()
    } else {
        return Err(Error::Invalid);
    };
    Ok((
        date.timestamp(),
        false,
        zone.clone(),
        format!("DTSTART;TZID={zone}:{naive}"),
    ))
}
fn normalize(v: &Value, calendar_id: &str, zone: &str, defaults: &Value) -> Result<Event> {
    let uid = v["iCalUID"]
        .as_str()
        .or_else(|| v["recurringEventId"].as_str())
        .or_else(|| v["id"].as_str())
        .ok_or(Error::Invalid)?
        .to_owned();
    let cancelled = v["status"] == "cancelled";
    let recurrence_id = if v.get("originalStartTime").is_some() {
        Some(google_time(&v["originalStartTime"], zone)?.0)
    } else {
        None
    };
    let start_value = if cancelled && v.get("start").is_none() {
        &v["originalStartTime"]
    } else {
        &v["start"]
    };
    let (start, all_day, timezone, start_rule) = google_time(start_value, zone)?;
    let end = if v.get("end").is_some() {
        google_time(&v["end"], zone)?.0
    } else {
        start
    };
    let reminders_value = if v["reminders"]["useDefault"] == true {
        defaults
    } else {
        &v["reminders"]["overrides"]
    };
    let reminders = if v.get("reminders").is_none() {
        calendar_core::Reminders::Unknown
    } else {
        let mins = reminders_value
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|x| x["method"] == "popup")
                    .filter_map(|x| x["minutes"].as_i64())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if mins.is_empty() {
            calendar_core::Reminders::Disabled
        } else {
            calendar_core::Reminders::Minutes(mins)
        }
    };
    Ok(Event {
        id: calendar_core::identity(&[calendar_id, v["id"].as_str().ok_or(Error::Invalid)?]),
        calendar_id: calendar_id.into(),
        uid,
        title: v["summary"].as_str().unwrap_or("").into(),
        description: v["description"].as_str().unwrap_or("").into(),
        location: v["location"].as_str().unwrap_or("").into(),
        organizer: v["organizer"]["email"].as_str().unwrap_or("").into(),
        attendees: v["attendees"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        v["email"].as_str().map(|e| {
                            format!(
                                "{} ({})",
                                e,
                                v["responseStatus"].as_str().unwrap_or("unknown")
                            )
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        start,
        end,
        all_day,
        timezone,
        recurrence_id,
        recurrence: v["recurrence"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        start_rule,
        cancelled,
        reminders,
        raw: v.to_string(),
    })
}
impl CalendarProvider for Google {
    fn sync(&self, context: &Context<'_>, credentials: &Credentials) -> Result<SyncResult> {
        let Credentials::Google {
            client_id,
            client_secret,
            refresh_token,
        } = credentials
        else {
            return Err(Error::Invalid);
        };
        let t = token(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
        ])?;
        let access =
            zeroize::Zeroizing::new(t["access_token"].as_str().ok_or(Error::OAuth)?.to_owned());
        let client = ReadClient::new(context.cancellation.clone())?;
        let (listed, _, _) = pages(&client, "users/me/calendarList", vec![], &access)?;
        let mut output = Vec::new();
        for source in listed {
            if source["accessRole"] == "freeBusyReader" {
                continue;
            }
            let sid = source["id"].as_str().ok_or(Error::Invalid)?;
            let mut calendar = calendar(
                context,
                sid,
                source["summary"].as_str().unwrap_or("Google calendar"),
            );
            let path = format!(
                "calendars/{}/events",
                url::form_urlencoded::byte_serialize(sid.as_bytes()).collect::<String>()
            );
            let base = vec![
                ("singleEvents", "false".into()),
                ("showDeleted", "true".into()),
                ("maxResults", "2500".into()),
            ];
            let mut query = base.clone();
            if let Some(c) = &calendar.cursor {
                query.push(("syncToken", c.clone()));
            }
            let (mut changes, mut cursor, reset) = pages(&client, &path, query, &access)?;
            let mut events: BTreeMap<String, Event> = if calendar.cursor.is_some() && !reset {
                context
                    .cached
                    .iter()
                    .filter(|e| e.calendar_id == calendar.id)
                    .map(|e| (e.id.clone(), e.clone()))
                    .collect()
            } else {
                BTreeMap::new()
            };
            if reset {
                let result = pages(&client, &path, base, &access)?;
                changes = result.0;
                cursor = result.1;
            }
            changes.sort_by_key(|v| v.get("recurringEventId").is_some());
            for v in changes {
                let id = calendar_core::identity(&[
                    &calendar.id,
                    v["id"].as_str().ok_or(Error::Invalid)?,
                ]);
                if v["status"] == "cancelled" && v.get("recurringEventId").is_none() {
                    events.remove(&id);
                    continue;
                }
                let mut e = normalize(
                    &v,
                    &calendar.id,
                    source["timeZone"].as_str().unwrap_or(context.timezone),
                    &source["defaultReminders"],
                )?;
                if let Some(parent) = v["recurringEventId"].as_str() {
                    let pid = calendar_core::identity(&[&calendar.id, parent]);
                    if let Some(master) = events.get(&pid) {
                        e.uid = master.uid.clone();
                    }
                }
                if let Some(original) = e.recurrence_id {
                    e.id =
                        calendar_core::identity(&[&e.calendar_id, &e.uid, &original.to_string()]);
                }
                events.insert(e.id.clone(), e);
            }
            calendar.cursor = Some(cursor.ok_or(Error::Invalid)?);
            calendar.last_sync = Some(Utc::now().timestamp());
            calendar.error = None;
            output.push(SyncedCalendar {
                calendar,
                events: events.into_values().collect(),
            });
        }
        Ok(SyncResult { calendars: output })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;
    #[test]
    fn scopes_are_readonly() {
        assert!(SCOPES.split_whitespace().all(|s| s.ends_with(".readonly")));
    }
    #[test]
    fn dates_are_real_dates() {
        let (t, all, _, _) =
            google_time(&json!({"date":"2026-11-01"}), "America/New_York").unwrap();
        assert!(all);
        assert_eq!(
            Utc.timestamp_opt(t, 0).unwrap().format("%H:%M").to_string(),
            "04:00"
        );
    }
}
