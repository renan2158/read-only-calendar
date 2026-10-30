//! UI-independent read-only calendar model, bounded recurrence and notification policy.
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use thiserror::Error;

pub const CONTRACT_VERSION: u32 = 1;
pub const MAX_FEED_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_EVENTS: usize = 25_000;
#[derive(Debug, Error)]
pub enum Error {
    #[error("The calendar contains an unsupported or invalid date, time zone, or recurrence.")]
    InvalidCalendar,
    #[error("The calendar exceeds the safety limits. The previous snapshot was retained.")]
    Limit,
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "kind", content = "minutes", rename_all = "camelCase")]
pub enum Reminders {
    Unknown,
    Disabled,
    Minutes(Vec<i64>),
}
impl Default for Reminders {
    fn default() -> Self {
        Self::Unknown
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub calendar_id: String,
    pub uid: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    pub timezone: String,
    pub description: String,
    pub location: String,
    pub attendees: Vec<String>,
    pub organizer: String,
    pub recurrence_id: Option<i64>,
    pub recurrence: Vec<String>,
    pub start_rule: String,
    pub cancelled: bool,
    pub reminders: Reminders,
    pub raw: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Calendar {
    pub id: String,
    pub account_id: String,
    pub source_id: String,
    pub name: String,
    pub color: String,
    pub visible: bool,
    pub last_sync: Option<i64>,
    pub error: Option<String>,
    pub cursor: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub provider: String,
    pub name: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub hide_details: bool,
    pub timezone: String,
    pub fallback_minutes: i64,
    pub all_day_hour: u32,
    pub background: bool,
    pub idle_minutes: u32,
    pub poll_minutes: u32,
    pub missed_minutes: i64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            hide_details: false,
            timezone: "UTC".into(),
            fallback_minutes: 10,
            all_day_hour: 9,
            background: false,
            idle_minutes: 15,
            poll_minutes: 15,
            missed_minutes: 60,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        self.timezone
            .parse::<Tz>()
            .map_err(|_| Error::InvalidCalendar)?;
        if !(0..=10080).contains(&self.fallback_minutes)
            || self.all_day_hour > 23
            || self.poll_minutes < 5
            || self.poll_minutes > 1440
            || self.idle_minutes > 1440
            || !(1..=1440).contains(&self.missed_minutes)
        {
            return Err(Error::InvalidCalendar);
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub id: String,
    pub event_id: String,
    pub due: i64,
    pub title: String,
    pub body: String,
}
pub fn identity(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_be_bytes());
        h.update(p.as_bytes());
    }
    hex::encode(h.finalize())
}
fn instant(value: &str, tz: &str) -> Result<i64> {
    if let Ok(d) = DateTime::parse_from_rfc3339(value) {
        return Ok(d.timestamp());
    }
    if value.ends_with('Z') {
        return NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
            .map(|d| d.and_utc().timestamp())
            .map_err(|_| Error::InvalidCalendar);
    }
    let zone: Tz = tz.parse().map_err(|_| Error::InvalidCalendar)?;
    let naive = if value.len() == 8 {
        NaiveDate::parse_from_str(value, "%Y%m%d")
            .map_err(|_| Error::InvalidCalendar)?
            .and_hms_opt(0, 0, 0)
            .ok_or(Error::InvalidCalendar)?
    } else {
        NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").map_err(|_| Error::InvalidCalendar)?
    };
    zone.from_local_datetime(&naive)
        .earliest()
        .map(|d| d.timestamp())
        .ok_or(Error::InvalidCalendar)
}
pub fn parse_time(value: &str, tz: &str) -> Result<i64> {
    instant(value, tz)
}
pub fn ics_text(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}
pub fn parse_ics(bytes: &[u8], calendar: &str, default_zone: &str) -> Result<Vec<Event>> {
    use ical::property::Property;
    if bytes.len() > MAX_FEED_BYTES {
        return Err(Error::Limit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidCalendar)?;
    if !text.trim_end().ends_with("END:VCALENDAR") {
        return Err(Error::InvalidCalendar);
    }
    let mut out = Vec::new();
    let mut found = false;
    for parsed in ical::IcalParser::new(Cursor::new(bytes)) {
        let cal = parsed.map_err(|_| Error::InvalidCalendar)?;
        found = true;
        let default_zone = cal
            .properties
            .iter()
            .find(|p| p.name == "X-WR-TIMEZONE")
            .and_then(|p| p.value.as_deref())
            .unwrap_or(default_zone);
        for e in cal.events {
            if out.len() >= MAX_EVENTS {
                return Err(Error::Limit);
            }
            let prop = |name: &str| e.properties.iter().find(|p| p.name == name);
            let val = |name: &str| prop(name).and_then(|p| p.value.clone()).unwrap_or_default();
            let tz_of = |p: &Property| {
                p.params
                    .as_ref()
                    .and_then(|ps| ps.iter().find(|(n, _)| n == "TZID"))
                    .and_then(|(_, v)| v.first())
                    .cloned()
                    .unwrap_or_else(|| default_zone.to_owned())
            };
            let uid = val("UID");
            if uid.is_empty() {
                return Err(Error::InvalidCalendar);
            }
            let cancelled = val("STATUS") == "CANCELLED";
            let sp = prop("DTSTART")
                .or_else(|| {
                    if cancelled {
                        prop("RECURRENCE-ID")
                    } else {
                        None
                    }
                })
                .ok_or(Error::InvalidCalendar)?;
            let sv = sp.value.as_deref().ok_or(Error::InvalidCalendar)?;
            let zone = tz_of(sp);
            let start = instant(sv, &zone)?;
            let all_day = sv.len() == 8;
            let end = match prop("DTEND") {
                Some(p) => instant(p.value.as_deref().ok_or(Error::InvalidCalendar)?, &tz_of(p))?,
                None => {
                    if let Some(p) = prop("DURATION") {
                        start
                            + duration_seconds(p.value.as_deref().ok_or(Error::InvalidCalendar)?)
                                .ok_or(Error::InvalidCalendar)?
                    } else if all_day {
                        let zone: Tz = zone.parse().map_err(|_| Error::InvalidCalendar)?;
                        let day = Utc
                            .timestamp_opt(start, 0)
                            .single()
                            .ok_or(Error::InvalidCalendar)?
                            .with_timezone(&zone)
                            .date_naive()
                            + chrono::Duration::days(1);
                        zone.from_local_datetime(
                            &day.and_hms_opt(0, 0, 0).ok_or(Error::InvalidCalendar)?,
                        )
                        .earliest()
                        .ok_or(Error::InvalidCalendar)?
                        .timestamp()
                    } else {
                        start
                    }
                }
            };
            if end < start {
                return Err(Error::InvalidCalendar);
            }
            let recurrence_id = prop("RECURRENCE-ID")
                .map(|p| instant(p.value.as_deref().unwrap_or(""), &tz_of(p)))
                .transpose()?;
            if let Some(p) = prop("RECURRENCE-ID") {
                if p.params
                    .as_ref()
                    .is_some_and(|ps| ps.iter().any(|(n, _)| n == "RANGE"))
                {
                    return Err(Error::InvalidCalendar);
                }
            }
            let mut recurrence = Vec::new();
            for p in &e.properties {
                if ["RRULE", "RDATE", "EXDATE", "EXRULE"].contains(&p.name.as_str()) {
                    let params = p
                        .params
                        .as_ref()
                        .map(|ps| {
                            ps.iter()
                                .map(|(n, vs)| format!(";{n}={}", vs.join(",")))
                                .collect::<String>()
                        })
                        .unwrap_or_default();
                    recurrence.push(format!(
                        "{}{params}:{}",
                        p.name,
                        p.value.as_deref().unwrap_or("")
                    ));
                }
            }
            let reminders = if e.alarms.is_empty() {
                Reminders::Unknown
            } else {
                let mut mins = Vec::new();
                for a in &e.alarms {
                    if a.properties
                        .iter()
                        .any(|p| p.name == "ACTION" && p.value.as_deref() == Some("DISPLAY"))
                    {
                        if let Some(p) = a.properties.iter().find(|p| p.name == "TRIGGER") {
                            if p.params.as_ref().is_some_and(|ps| {
                                ps.iter()
                                    .any(|(n, v)| n == "RELATED" && v.iter().any(|x| x == "END"))
                            }) {
                                return Err(Error::InvalidCalendar);
                            }
                            let v = p.value.as_deref().unwrap_or("");
                            if let Some(m) = relative_minutes(v) {
                                mins.push(m);
                            } else {
                                return Err(Error::InvalidCalendar);
                            }
                        }
                    }
                }
                if mins.is_empty() {
                    Reminders::Disabled
                } else {
                    Reminders::Minutes(mins)
                }
            };
            let start_rule = if all_day {
                format!("DTSTART;TZID={zone}:{sv}T000000")
            } else if sv.ends_with('Z') {
                format!("DTSTART:{sv}")
            } else {
                format!("DTSTART;TZID={zone}:{sv}")
            };
            let key = identity(&[
                calendar,
                &uid,
                &recurrence_id.map(|x| x.to_string()).unwrap_or_default(),
            ]);
            out.push(Event {
                id: key,
                calendar_id: calendar.into(),
                uid,
                title: ics_text(&val("SUMMARY")),
                start,
                end,
                all_day,
                timezone: zone,
                description: ics_text(&val("DESCRIPTION")),
                location: ics_text(&val("LOCATION")),
                organizer: val("ORGANIZER"),
                attendees: e
                    .properties
                    .iter()
                    .filter(|p| p.name == "ATTENDEE")
                    .filter_map(|p| p.value.clone())
                    .collect(),
                recurrence_id,
                recurrence,
                start_rule,
                cancelled,
                reminders,
                raw: format!(
                    "BEGIN:VEVENT\n{}\n{}\nEND:VEVENT",
                    e.properties
                        .iter()
                        .map(|p| {
                            let params = p
                                .params
                                .as_ref()
                                .map(|ps| {
                                    ps.iter()
                                        .map(|(n, v)| format!(";{n}={}", v.join(",")))
                                        .collect::<String>()
                                })
                                .unwrap_or_default();
                            format!("{}{params}:{}", p.name, p.value.as_deref().unwrap_or(""))
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    e.alarms
                        .iter()
                        .map(|a| format!(
                            "BEGIN:VALARM\n{}\nEND:VALARM",
                            a.properties
                                .iter()
                                .map(|p| format!("{}:{}", p.name, p.value.as_deref().unwrap_or("")))
                                .collect::<Vec<_>>()
                                .join("\n")
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            });
        }
    }
    if !found {
        return Err(Error::InvalidCalendar);
    }
    Ok(out)
}
fn duration_seconds(s: &str) -> Option<i64> {
    let mut total = 0i64;
    let mut number = String::new();
    if !s.starts_with('P') {
        return None;
    }
    for c in s[1..].chars() {
        if c.is_ascii_digit() {
            number.push(c);
        } else if c == 'T' {
            continue;
        } else {
            let n = number.parse::<i64>().ok()?;
            number.clear();
            let factor = match c {
                'W' => 604800,
                'D' => 86400,
                'H' => 3600,
                'M' => 60,
                'S' => 1,
                _ => return None,
            };
            total = total.checked_add(n.checked_mul(factor)?)?;
        }
    }
    if !number.is_empty() || total > 366 * 86400 {
        return None;
    }
    Some(total)
}
fn relative_minutes(s: &str) -> Option<i64> {
    let before = s.starts_with('-');
    let seconds = duration_seconds(s.strip_prefix('-').unwrap_or(s))?;
    if seconds % 60 != 0 {
        return None;
    }
    Some(if before { seconds / 60 } else { -seconds / 60 })
}
/// Expand at most one year of occurrences with a strict total occurrence ceiling.
pub fn expand(events: &[Event], from: i64, to: i64) -> Result<Vec<Event>> {
    if to <= from || to - from > 370 * 86400 {
        return Err(Error::Limit);
    }
    let mut overrides = BTreeMap::new();
    for e in events {
        if let Some(r) = e.recurrence_id {
            overrides.insert((e.calendar_id.clone(), e.uid.clone(), r), e);
        }
    }
    let mut result = Vec::new();
    let budget = std::time::Instant::now();
    for e in events {
        if budget.elapsed() > std::time::Duration::from_secs(3) {
            return Err(Error::Limit);
        }
        if e.cancelled {
            continue;
        }
        if e.recurrence_id.is_some() || e.recurrence.is_empty() {
            if e.start < to && e.end.max(e.start + 1) > from {
                result.push(e.clone());
            }
            continue;
        }
        let text = format!("{}\n{}", e.start_rule, e.recurrence.join("\n"));
        let set = text
            .parse::<rrule::RRuleSet>()
            .map_err(|_| Error::InvalidCalendar)?;
        let upper = Utc
            .timestamp_opt(to, 0)
            .single()
            .ok_or(Error::InvalidCalendar)?
            .with_timezone(&rrule::Tz::UTC);
        // all() internally limits iteration as well as returned dates; never unbounded iteration.
        let expanded = set.before(upper).all(25_000);
        if expanded.limited {
            return Err(Error::Limit);
        }
        for d in expanded.dates {
            let start = d.timestamp();
            if start + (e.end - e.start).max(1) <= from {
                continue;
            }
            if overrides.contains_key(&(e.calendar_id.clone(), e.uid.clone(), start)) {
                continue;
            }
            let mut instance = e.clone();
            instance.id = identity(&[&e.calendar_id, &e.uid, &start.to_string()]);
            instance.start = start;
            instance.end = if e.all_day {
                let zone: Tz = e.timezone.parse().map_err(|_| Error::InvalidCalendar)?;
                let original_start = Utc
                    .timestamp_opt(e.start, 0)
                    .single()
                    .ok_or(Error::InvalidCalendar)?
                    .with_timezone(&zone)
                    .date_naive();
                let original_end = Utc
                    .timestamp_opt(e.end, 0)
                    .single()
                    .ok_or(Error::InvalidCalendar)?
                    .with_timezone(&zone)
                    .date_naive();
                let days = (original_end - original_start).num_days().max(1);
                let end_date = d.with_timezone(&zone).date_naive() + chrono::Duration::days(days);
                zone.from_local_datetime(
                    &end_date
                        .and_hms_opt(0, 0, 0)
                        .ok_or(Error::InvalidCalendar)?,
                )
                .earliest()
                .ok_or(Error::InvalidCalendar)?
                .timestamp()
            } else {
                start + (e.end - e.start)
            };
            instance.recurrence.clear();
            result.push(instance);
            if result.len() > MAX_EVENTS {
                return Err(Error::Limit);
            }
        }
    }
    result.sort_by_key(|e| e.start);
    if result.len() > MAX_EVENTS {
        return Err(Error::Limit);
    }
    Ok(result)
}
pub fn alerts(events: &[Event], settings: &Settings, from: i64, to: i64) -> Result<Vec<Alert>> {
    settings.validate()?;
    let zone: Tz = settings
        .timezone
        .parse()
        .map_err(|_| Error::InvalidCalendar)?;
    let mut alerts = Vec::new();
    let mut seen = BTreeSet::new();
    for e in events {
        if e.cancelled {
            continue;
        }
        let start = if e.all_day {
            let original: Tz = e.timezone.parse().map_err(|_| Error::InvalidCalendar)?;
            let date = Utc
                .timestamp_opt(e.start, 0)
                .single()
                .ok_or(Error::InvalidCalendar)?
                .with_timezone(&original)
                .date_naive();
            zone.from_local_datetime(
                &date
                    .and_hms_opt(settings.all_day_hour, 0, 0)
                    .ok_or(Error::InvalidCalendar)?,
            )
            .earliest()
            .ok_or(Error::InvalidCalendar)?
            .timestamp()
        } else {
            e.start
        };
        let mut offsets = match &e.reminders {
            Reminders::Unknown => vec![settings.fallback_minutes],
            Reminders::Disabled => vec![],
            Reminders::Minutes(m) => m.clone(),
        };
        offsets.push(0);
        for offset in offsets {
            let due = start - offset * 60;
            if due < from || due > to {
                continue;
            }
            let id = identity(&[&e.calendar_id, &e.id, &due.to_string()]);
            if !seen.insert(id.clone()) {
                continue;
            }
            let time = Utc
                .timestamp_opt(start, 0)
                .single()
                .ok_or(Error::InvalidCalendar)?
                .with_timezone(&zone)
                .format("%H:%M")
                .to_string();
            let body = if e.all_day {
                format!("All day · {time}")
            } else if offset == 0 {
                format!("Starts now · {time}")
            } else if offset > 0 {
                format!("Starts in {offset} minutes · {time}")
            } else {
                format!("Started {} minutes ago · {time}", -offset)
            };
            alerts.push(Alert {
                id,
                event_id: e.id.clone(),
                due,
                title: if settings.hide_details {
                    "Upcoming event".into()
                } else if e.title.is_empty() {
                    "Untitled event".into()
                } else {
                    e.title.clone()
                },
                body,
            });
        }
    }
    alerts.sort_by_key(|a| a.due);
    Ok(alerts)
}
#[cfg(test)]
mod tests {
    use super::*;
    const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:test\r\nDTSTART;TZID=America/New_York:20261031T090000\r\nDTEND;TZID=America/New_York:20261031T100000\r\nSUMMARY:Private title\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    #[test]
    fn dst_recurrence() {
        let e = parse_ics(ICS.as_bytes(), "c", "UTC").unwrap();
        let out = expand(&e, 1793404800, 1793836800).unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[1].start - out[0].start, 25 * 3600);
    }
    #[test]
    fn hide_keeps_time_and_deduplicates() {
        let mut e = parse_ics(ICS.as_bytes(), "c", "UTC").unwrap();
        e[0].reminders = Reminders::Minutes(vec![0, 0]);
        let settings = Settings {
            hide_details: true,
            ..Default::default()
        };
        let a = alerts(&e, &settings, e[0].start, e[0].start).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].title, "Upcoming event");
        assert!(a[0].body.contains("13:00"));
    }
    #[test]
    fn invalid_feed_rejected() {
        assert!(parse_ics(b"<html>", "c", "UTC").is_err());
    }
    #[test]
    fn identities_are_namespaced() {
        assert_ne!(identity(&["a", "event"]), identity(&["b", "event"]));
        assert_ne!(identity(&["ab", "c"]), identity(&["a", "bc"]));
    }
}
#[cfg(test)]
mod regression_tests {
    use super::*;
    fn feed(events: &str) -> Vec<Event> {
        parse_ics(
            format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{events}END:VCALENDAR\r\n").as_bytes(),
            "calendar",
            "UTC",
        )
        .unwrap()
    }
    #[test]
    fn cancellation_and_moved_occurrence() {
        let e = feed(
            "BEGIN:VEVENT\r\nUID:series\r\nDTSTART:20261101T090000Z\r\nDTEND:20261101T100000Z\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:series\r\nRECURRENCE-ID:20261102T090000Z\r\nSTATUS:CANCELLED\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:series\r\nRECURRENCE-ID:20261103T090000Z\r\nDTSTART:20261103T110000Z\r\nDTEND:20261103T120000Z\r\nEND:VEVENT\r\n",
        );
        let a = expand(&e, 1793491200, 1793836800).unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!(a[1].start, parse_time("20261103T110000Z", "UTC").unwrap());
    }
    #[test]
    fn explicit_disabled_reminders_only_emit_start() {
        let mut e = feed("BEGIN:VEVENT\r\nUID:x\r\nDTSTART:20261101T090000Z\r\nEND:VEVENT\r\n");
        e[0].reminders = Reminders::Disabled;
        let a = alerts(&e, &Settings::default(), e[0].start - 3600, e[0].start).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].due, e[0].start);
    }
    #[test]
    fn all_day_recurrence_keeps_exclusive_end_across_dst() {
        let e = feed(
            "BEGIN:VEVENT\r\nUID:x\r\nDTSTART;VALUE=DATE:20261031\r\nDTEND;VALUE=DATE:20261101\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEND:VEVENT\r\n",
        );
        let mut e = e;
        for item in &mut e {
            item.timezone = "America/New_York".into();
            item.start_rule = "DTSTART;TZID=America/New_York:20261031T000000".into();
            item.start = parse_time("20261031", "America/New_York").unwrap();
            item.end = parse_time("20261101", "America/New_York").unwrap();
        }
        let a = expand(&e, 1793318400, 1793923200).unwrap();
        assert_eq!(a.len(), 3);
        assert_eq!(a[1].end - a[1].start, 25 * 3600);
        assert_eq!(a[2].end - a[2].start, 24 * 3600);
    }
    #[test]
    fn duration_is_honored() {
        let e = feed(
            "BEGIN:VEVENT\r\nUID:duration\r\nDTSTART:20261101T090000Z\r\nDURATION:PT1H30M\r\nEND:VEVENT\r\n",
        );
        assert_eq!(e[0].end - e[0].start, 5400);
    }
}
