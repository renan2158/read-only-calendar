use super::*;
pub struct Ics;
impl CalendarProvider for Ics {
    fn sync(&self, context: &Context<'_>, credentials: &Credentials) -> Result<SyncResult> {
        let (url, name) = match credentials {
            Credentials::Ics { url } => (url, "Subscribed calendar"),
            Credentials::Proton { url } => (url, "Proton Calendar"),
            _ => return Err(Error::Invalid),
        };
        let mut calendar = calendar(context, "feed", name);
        let client = ReadClient::new(context.cancellation.clone())?;
        let validators: serde_json::Value = calendar
            .cursor
            .as_deref()
            .and_then(|c| serde_json::from_str(c).ok())
            .unwrap_or_default();
        let mut headers = Vec::new();
        if let Some(v) = validators["etag"].as_str() {
            headers.push(("If-None-Match", v));
        }
        if let Some(v) = validators["modified"].as_str() {
            headers.push(("If-Modified-Since", v));
        }
        let r = client.request(ReadMethod::Get, url, None, None, &headers, None)?;
        let events = if r.status == 304 {
            context
                .cached
                .iter()
                .filter(|e| e.calendar_id == calendar.id)
                .cloned()
                .collect()
        } else {
            if r.status != 200 {
                return Err(Error::Network);
            }
            let events = calendar_core::parse_ics(&r.bytes, &calendar.id, context.timezone)
                .map_err(|_| Error::Invalid)?;
            calendar.cursor =
                Some(serde_json::json!({"etag":r.etag,"modified":r.modified}).to_string());
            events
        };
        calendar.last_sync = Some(chrono::Utc::now().timestamp());
        calendar.error = None;
        Ok(SyncResult {
            calendars: vec![SyncedCalendar { calendar, events }],
        })
    }
}
