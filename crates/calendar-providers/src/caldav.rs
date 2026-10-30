use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub struct CalDav;
#[derive(Default, Serialize, Deserialize)]
struct Cursor {
    version: u32,
    token: Option<String>,
    etags: BTreeMap<String, String>,
}
#[derive(Default)]
struct Resource {
    href: String,
    etag: Option<String>,
    data: Option<String>,
    deleted: bool,
}
fn escape(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn xml_response(r: Response) -> Result<String> {
    if r.status != 207 {
        return Err(Error::Invalid);
    }
    String::from_utf8(r.bytes).map_err(|_| Error::Invalid)
}
fn propfind(
    client: &ReadClient,
    url: &str,
    user: &str,
    pass: &str,
    depth: &str,
    properties: &str,
) -> Result<String> {
    let b = format!(
        "<d:propfind xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\"><d:prop>{properties}</d:prop></d:propfind>"
    );
    xml_response(client.request(
        ReadMethod::Propfind,
        url,
        Some((user, pass)),
        None,
        &[
            ("Depth", depth),
            ("Content-Type", "application/xml; charset=utf-8"),
        ],
        Some(&b),
    )?)
}
fn href(xml: &str, property: &str) -> Result<String> {
    let doc = roxmltree::Document::parse(xml).map_err(|_| Error::Invalid)?;
    doc.descendants()
        .find(|n| n.tag_name().name() == property)
        .and_then(|n| n.descendants().find(|h| h.tag_name().name() == "href"))
        .and_then(|n| n.text())
        .map(str::to_owned)
        .ok_or(Error::Invalid)
}
fn join(base: &str, path: &str) -> Result<String> {
    let b = ReadClient::endpoint(base)?;
    let u = b.join(path).map_err(|_| Error::Invalid)?;
    if b.origin() != u.origin() {
        return Err(Error::Invalid);
    }
    Ok(u.to_string())
}
fn resources(xml: &str, base: &str) -> Result<(Vec<Resource>, Option<String>)> {
    let doc = roxmltree::Document::parse(xml).map_err(|_| Error::Invalid)?;
    let token = doc
        .root_element()
        .children()
        .find(|n| n.tag_name().name() == "sync-token")
        .and_then(|n| n.text())
        .map(str::to_owned);
    let mut resources = Vec::new();
    for n in doc
        .descendants()
        .filter(|n| n.tag_name().name() == "response")
    {
        let Some(h) = n
            .children()
            .find(|n| n.tag_name().name() == "href")
            .and_then(|n| n.text())
        else {
            return Err(Error::Invalid);
        };
        let mut resource = Resource {
            href: join(base, h)?,
            ..Default::default()
        };
        if let Some(status) = n
            .children()
            .find(|n| n.tag_name().name() == "status")
            .and_then(|n| n.text())
        {
            if status.contains(" 404 ") {
                resource.deleted = true;
            } else if !status.contains(" 200 ") {
                return Err(Error::Invalid);
            }
        }
        for propstat in n.children().filter(|n| n.tag_name().name() == "propstat") {
            let status = propstat
                .children()
                .find(|n| n.tag_name().name() == "status")
                .and_then(|n| n.text())
                .unwrap_or("");
            if status.contains(" 404 ") {
                continue;
            }
            if !status.contains(" 200 ") {
                return Err(Error::Invalid);
            }
            for p in propstat.descendants() {
                match p.tag_name().name() {
                    "getetag" => resource.etag = p.text().map(str::to_owned),
                    "calendar-data" => resource.data = p.text().map(str::to_owned),
                    _ => {}
                }
            }
        }
        if resource.etag.is_some() || resource.data.is_some() || resource.deleted {
            resources.push(resource);
        }
    }
    Ok((resources, token))
}
fn event_href(e: &Event) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(&e.raw)
        .ok()?
        .get("href")?
        .as_str()
        .map(str::to_owned)
}
fn multiget_body(hrefs: &[String]) -> String {
    format!(
        "<c:calendar-multiget xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\"><d:prop><d:getetag/><c:calendar-data/></d:prop>{}</c:calendar-multiget>",
        hrefs
            .iter()
            .map(|h| format!("<d:href>{}</d:href>", escape(h)))
            .collect::<String>()
    )
}
fn read_calendar(
    client: &ReadClient,
    context: &Context<'_>,
    calendar: &Calendar,
    user: &str,
    pass: &str,
) -> Result<(Vec<Event>, Cursor)> {
    let source = &calendar.source_id;
    let old = calendar
        .cursor
        .as_deref()
        .and_then(|s| serde_json::from_str::<Cursor>(s).ok())
        .filter(|c| c.version == 1);
    let mut events: Vec<Event> = context
        .cached
        .iter()
        .filter(|e| e.calendar_id == calendar.id)
        .cloned()
        .collect();
    let mut cursor = Cursor {
        version: 1,
        ..Default::default()
    };
    let mut incremental = false;
    let mut changes = Vec::new();
    if let Some(old) = &old {
        if let Some(token) = &old.token {
            let body = format!(
                "<d:sync-collection xmlns:d=\"DAV:\"><d:sync-token>{}</d:sync-token><d:sync-level>1</d:sync-level><d:prop><d:getetag/></d:prop></d:sync-collection>",
                escape(token)
            );
            if let Ok(r) = client.request(
                ReadMethod::Report,
                source,
                Some((user, pass)),
                None,
                &[("Content-Type", "application/xml")],
                Some(&body),
            ) {
                if r.status == 207 {
                    let text = xml_response(r)?;
                    let result = resources(&text, source)?;
                    changes = result.0;
                    cursor.token = result.1;
                    cursor.etags = old.etags.clone();
                    incremental = cursor.token.is_some();
                }
            }
        }
    }
    if !incremental {
        let text = propfind(
            client,
            source,
            user,
            pass,
            "1",
            "<d:getetag/><d:sync-token/>",
        )?;
        let doc = roxmltree::Document::parse(&text).map_err(|_| Error::Invalid)?;
        cursor.token = doc
            .descendants()
            .find(|n| n.tag_name().name() == "sync-token")
            .and_then(|n| n.text())
            .map(str::to_owned);
        let (inventory, _) = resources(&text, source)?;
        changes = inventory
            .into_iter()
            .filter(|r| r.href.trim_end_matches('/') != source.trim_end_matches('/'))
            .collect();
        let live: Vec<_> = changes.iter().map(|r| r.href.clone()).collect();
        events.retain(|e| event_href(e).is_some_and(|h| live.contains(&h)));
        for r in &changes {
            if let Some(etag) = &r.etag {
                cursor.etags.insert(r.href.clone(), etag.clone());
            }
        }
    }
    let changed: Vec<_> = changes
        .iter()
        .filter(|r| {
            !r.deleted
                && (!incremental
                    || old.as_ref().and_then(|o| o.etags.get(&r.href)) != r.etag.as_ref())
        })
        .map(|r| r.href.clone())
        .collect();
    for r in &changes {
        if r.deleted {
            events.retain(|e| event_href(e).as_deref() != Some(&r.href));
            cursor.etags.remove(&r.href);
        } else if let Some(etag) = &r.etag {
            cursor.etags.insert(r.href.clone(), etag.clone());
        }
    }
    for chunk in changed.chunks(100) {
        let body = multiget_body(chunk);
        let text = xml_response(client.request(
            ReadMethod::Report,
            source,
            Some((user, pass)),
            None,
            &[("Content-Type", "application/xml")],
            Some(&body),
        )?)?;
        let (resources, _) = resources(&text, source)?;
        if resources.len() != chunk.len() {
            return Err(Error::Invalid);
        }
        for r in resources {
            if !chunk.contains(&r.href) || r.deleted {
                return Err(Error::Invalid);
            }
            let data = r.data.ok_or(Error::Invalid)?;
            let mut parsed =
                calendar_core::parse_ics(data.as_bytes(), &calendar.id, context.timezone)
                    .map_err(|_| Error::Invalid)?;
            events.retain(|e| event_href(e).as_deref() != Some(&r.href));
            for e in &mut parsed {
                e.raw = serde_json::json!({"href":r.href,"etag":r.etag,"ical":e.raw}).to_string();
            }
            events.extend(parsed);
        }
    }
    if events.len() > calendar_core::MAX_EVENTS {
        return Err(Error::Limit);
    }
    Ok((events, cursor))
}
impl CalendarProvider for CalDav {
    fn sync(&self, context: &Context<'_>, credentials: &Credentials) -> Result<SyncResult> {
        let Credentials::Caldav {
            url,
            username,
            password,
        } = credentials
        else {
            return Err(Error::Invalid);
        };
        let client = ReadClient::new(context.cancellation.clone())?;
        let p = propfind(
            &client,
            url,
            username,
            password,
            "0",
            "<d:current-user-principal/>",
        )?;
        let principal = join(url, &href(&p, "current-user-principal")?)?;
        let p = propfind(
            &client,
            &principal,
            username,
            password,
            "0",
            "<c:calendar-home-set/>",
        )?;
        let home = join(url, &href(&p, "calendar-home-set")?)?;
        let p = propfind(
            &client,
            &home,
            username,
            password,
            "1",
            "<d:resourcetype/><d:displayname/><d:current-user-privilege-set/>",
        )?;
        let doc = roxmltree::Document::parse(&p).map_err(|_| Error::Invalid)?;
        let mut calendars = Vec::new();
        for n in doc
            .descendants()
            .filter(|n| n.tag_name().name() == "response")
        {
            if !n.descendants().any(|n| {
                n.tag_name().namespace() == Some("urn:ietf:params:xml:ns:caldav")
                    && n.tag_name().name() == "calendar"
            }) {
                continue;
            }
            if calendars.len() >= 100 {
                return Err(Error::Limit);
            }
            let source = n
                .children()
                .find(|n| n.tag_name().name() == "href")
                .and_then(|n| n.text())
                .ok_or(Error::Invalid)?;
            let source = join(url, source)?;
            let name = n
                .descendants()
                .find(|n| n.tag_name().name() == "displayname")
                .and_then(|n| n.text())
                .unwrap_or("CalDAV calendar");
            let mut calendar = calendar(context, &source, name);
            let (events, cursor) = read_calendar(&client, context, &calendar, username, password)?;
            calendar.cursor = Some(serde_json::to_string(&cursor).map_err(|_| Error::Invalid)?);
            calendar.last_sync = Some(chrono::Utc::now().timestamp());
            calendar.error = None;
            calendars.push(SyncedCalendar { calendar, events });
        }
        Ok(SyncResult { calendars })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn href_escaping() {
        let b = multiget_body(&["/a?x=1&y=2".into()]);
        assert!(b.contains("&amp;"));
    }
    #[test]
    fn cross_origin_discovery_refused() {
        assert!(join("https://example.com/", "https://other.example/user").is_err());
    }
    #[test]
    fn tombstones() {
        let xml = "<d:multistatus xmlns:d='DAV:'><d:response><d:href>/a.ics</d:href><d:status>HTTP/1.1 404 Not Found</d:status></d:response><d:sync-token>new</d:sync-token></d:multistatus>";
        let (r, t) = resources(xml, "https://example.com/").unwrap();
        assert!(r[0].deleted);
        assert_eq!(t.as_deref(), Some("new"));
    }
}
