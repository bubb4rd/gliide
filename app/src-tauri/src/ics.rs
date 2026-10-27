//! Calendar invites (`.ics`, RFC 5545) read into a round's fields, with no model call: the first event's start (in its
//! `TZID`, UTC, floating in the Mac's zone, or an all-day date), its length (`DTEND` or `DURATION`), its title's kind
//! by keywords, where (`LOCATION`, else a meeting link, else `URL`), its organizer and its description.

use chrono::{
    DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, SecondsFormat, TimeDelta,
    TimeZone, Utc,
};
use chrono_tz::Tz;

use crate::dto::{RoundDraft, RoundKind};

/// The most of a description kept as notes.
const NOTES_CHARS: usize = 400;

/// Round kinds by keywords in an event's title, the more specific first ("Technical phone screen" is technical).
const KINDS: [(RoundKind, &[&str]); 8] = [
    (RoundKind::TakeHome, &["take-home", "take home", "takehome"]),
    (
        RoundKind::SystemDesign,
        &["system design", "systems design"],
    ),
    (
        RoundKind::Technical,
        &["technical", "coding", "pair programming", "tech screen"],
    ),
    (RoundKind::HiringManager, &["hiring manager"]),
    (RoundKind::Behavioral, &["behavioral", "behavioural"]),
    (
        RoundKind::RecruiterScreen,
        &[
            "recruiter",
            "phone screen",
            "phone interview",
            "screening",
            "intro call",
        ],
    ),
    (RoundKind::Onsite, &["onsite", "on-site", "on site"]),
    (RoundKind::Final, &["final"]),
];

/// Where video calls are hosted: a link to one in a description is the round's place.
const MEETING_HOSTS: [&str; 10] = [
    "zoom.us/",
    "meet.google.com/",
    "teams.microsoft.com/",
    "teams.live.com/",
    "webex.com/",
    "whereby.com/",
    "chime.aws/",
    "gotomeeting.com/",
    "bluejeans.com/",
    "around.co/",
];

/// Calendar apps' own properties for an event's video call.
const MEETING_PROPS: [&str; 3] = [
    "X-GOOGLE-CONFERENCE",
    "X-MICROSOFT-SKYPETEAMSMEETINGURL",
    "X-MICROSOFT-ONLINEMEETINGCONFLINK",
];

/// Outlook names zones the Windows way; the common ones, as IANA zones.
const WINDOWS_ZONES: [(&str, &str); 24] = [
    ("Eastern Standard Time", "America/New_York"),
    ("Central Standard Time", "America/Chicago"),
    ("Mountain Standard Time", "America/Denver"),
    ("US Mountain Standard Time", "America/Phoenix"),
    ("Pacific Standard Time", "America/Los_Angeles"),
    ("Alaskan Standard Time", "America/Anchorage"),
    ("Hawaiian Standard Time", "Pacific/Honolulu"),
    ("Atlantic Standard Time", "America/Halifax"),
    ("Newfoundland Standard Time", "America/St_Johns"),
    ("GMT Standard Time", "Europe/London"),
    ("Greenwich Standard Time", "Atlantic/Reykjavik"),
    ("W. Europe Standard Time", "Europe/Berlin"),
    ("Romance Standard Time", "Europe/Paris"),
    ("Central Europe Standard Time", "Europe/Budapest"),
    ("Central European Standard Time", "Europe/Warsaw"),
    ("E. Europe Standard Time", "Europe/Chisinau"),
    ("FLE Standard Time", "Europe/Kiev"),
    ("Israel Standard Time", "Asia/Jerusalem"),
    ("India Standard Time", "Asia/Kolkata"),
    ("China Standard Time", "Asia/Shanghai"),
    ("Singapore Standard Time", "Asia/Singapore"),
    ("Tokyo Standard Time", "Asia/Tokyo"),
    ("AUS Eastern Standard Time", "Australia/Sydney"),
    ("New Zealand Standard Time", "Pacific/Auckland"),
];

/// One content line: its name (uppercase), parameters (names uppercase, values unquoted) and raw value.
struct Prop {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

impl Prop {
    fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// The first event in `ics` as a round's fields; `None` when it holds no event.
pub fn read(ics: &str) -> Option<RoundDraft> {
    let props = first_event(ics)?;
    let get = |name: &str| props.iter().find(|p| p.name == name);
    let text_of = |name: &str| get(name).map(|p| unescape(&p.value).trim().to_owned());

    let start = get("DTSTART").and_then(time);
    let minutes = start.and_then(|s| {
        let length = match get("DTEND").and_then(time) {
            Some(end) => end - s,
            None => duration(&get("DURATION")?.value)?,
        };
        u32::try_from(length.num_minutes())
            .ok()
            .filter(|m| (1..=600).contains(m))
    });
    let description = text_of("DESCRIPTION").unwrap_or_default();
    let conference = MEETING_PROPS
        .iter()
        .find_map(|name| text_of(name).filter(|v| !v.is_empty()))
        .or_else(|| meeting_link(&description));
    let location = text_of("LOCATION").filter(|l| !l.is_empty());
    let place = match location {
        Some(l) if l.contains("://") || conference.is_none() => l,
        location => conference
            .or_else(|| text_of("URL").filter(|u| !u.is_empty()))
            .or(location)
            .unwrap_or_default(),
    };
    Some(RoundDraft {
        id: None,
        kind: text_of("SUMMARY").and_then(|s| kind(&s)),
        starts_at: start.map(|s| s.to_rfc3339_opts(SecondsFormat::Secs, true)),
        minutes,
        with: get("ORGANIZER")
            .and_then(|p| p.param("CN"))
            .map(|cn| unescape(cn).trim().to_owned())
            .unwrap_or_default(),
        place,
        notes: notes(&description),
    })
}

/// The first `VEVENT`'s own properties (not its alarms'); `None` without one.
fn first_event(ics: &str) -> Option<Vec<Prop>> {
    let mut props = Vec::new();
    let mut inside = false;
    // Components open inside the event (`VALARM`).
    let mut depth = 0usize;
    for line in unfold(ics) {
        let Some(p) = prop(&line) else { continue };
        match p.name.as_str() {
            "BEGIN" if !inside => inside = p.value.trim().eq_ignore_ascii_case("VEVENT"),
            "BEGIN" => depth += 1,
            "END" if inside && depth == 0 => return Some(props),
            "END" if inside => depth -= 1,
            _ if inside && depth == 0 => props.push(p),
            _ => {}
        }
    }
    inside.then_some(props)
}

/// The content lines, folded lines joined (a line starting with a space or tab continues the one before).
fn unfold(ics: &str) -> Vec<String> {
    let ics = ics.strip_prefix('\u{feff}').unwrap_or(ics);
    let mut lines: Vec<String> = Vec::new();
    for raw in ics.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        match (raw.strip_prefix([' ', '\t']), lines.last_mut()) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ if raw.is_empty() => {}
            _ => lines.push(raw.to_owned()),
        }
    }
    lines
}

/// `NAME;PARAM=value;PARAM="quoted:value":value`.
fn prop(line: &str) -> Option<Prop> {
    let mut quoted = false;
    let (colon, _) = line.char_indices().find(|&(_, c)| {
        if c == '"' {
            quoted = !quoted;
        }
        c == ':' && !quoted
    })?;
    let mut parts = split_unquoted(&line[..colon], ';').into_iter();
    let name = parts.next()?.trim().to_ascii_uppercase();
    let params = parts
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((
                k.trim().to_ascii_uppercase(),
                v.trim().trim_matches('"').to_owned(),
            ))
        })
        .collect();
    Some(Prop {
        name,
        params,
        value: line[colon + 1..].to_owned(),
    })
}

fn split_unquoted(s: &str, at: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut quoted, mut from) = (false, 0);
    for (i, c) in s.char_indices() {
        if c == '"' {
            quoted = !quoted;
        } else if c == at && !quoted {
            parts.push(&s[from..i]);
            from = i + c.len_utf8();
        }
    }
    parts.push(&s[from..]);
    parts
}

/// A TEXT value unescaped: `\n` a line break, `\,` `\;` `\\` the character.
fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(c) => out.push(c),
            None => {}
        }
    }
    out
}

/// A `DTSTART`/`DTEND`: in its `TZID`, UTC (`…Z`), floating (the Mac's zone), or a date (its midnight, the Mac's zone).
fn time(p: &Prop) -> Option<DateTime<FixedOffset>> {
    let v = p.value.trim();
    if p.param("VALUE") == Some("DATE") || v.len() == 8 {
        let day = NaiveDate::parse_from_str(v, "%Y%m%d").ok()?;
        return in_zone(&Local, day.and_time(NaiveTime::MIN));
    }
    if let Some(utc) = v.strip_suffix(['Z', 'z']) {
        let at = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        return Some(Utc.from_utc_datetime(&at).fixed_offset());
    }
    let at = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    match p.param("TZID").and_then(zone) {
        Some(tz) => in_zone(&tz, at),
        None => in_zone(&Local, at),
    }
}

/// `at` on the wall clock of `zone`: the earlier of a repeated hour; an hour on, for a time daylight saving skips.
fn in_zone<Z: TimeZone>(zone: &Z, at: NaiveDateTime) -> Option<DateTime<FixedOffset>> {
    zone.from_local_datetime(&at)
        .earliest()
        .or_else(|| {
            zone.from_local_datetime(&(at + TimeDelta::hours(1)))
                .earliest()
        })
        .map(|d| d.fixed_offset())
}

/// A `TZID` as an IANA zone: as written, the zone at the end of a path (`/mozilla.org/…/America/New_York`), or an
/// Outlook (Windows) zone name.
fn zone(tzid: &str) -> Option<Tz> {
    let id = tzid.trim();
    if let Ok(tz) = id.parse() {
        return Some(tz);
    }
    let parts: Vec<&str> = id.split('/').filter(|p| !p.is_empty()).collect();
    for n in [3, 2] {
        if parts.len() >= n
            && let Ok(tz) = parts[parts.len() - n..].join("/").parse()
        {
            return Some(tz);
        }
    }
    WINDOWS_ZONES
        .iter()
        .find(|(windows, _)| windows.eq_ignore_ascii_case(id))
        .and_then(|(_, iana)| iana.parse().ok())
}

/// A `DURATION` (`PT45M`, `PT1H30M`, `P1D`, `P1W`); `None` for a negative or malformed one.
fn duration(v: &str) -> Option<TimeDelta> {
    let v = v.trim();
    let rest = v.strip_prefix('+').unwrap_or(v).strip_prefix('P')?;
    let (mut secs, mut digits, mut in_time, mut any) = (0i64, String::new(), false, false);
    for c in rest.chars() {
        match c {
            '0'..='9' => digits.push(c),
            'T' if !in_time && digits.is_empty() => in_time = true,
            'W' | 'D' | 'H' | 'M' | 'S' => {
                let n: i64 = digits.parse().ok()?;
                digits.clear();
                any = true;
                secs += n * match (c, in_time) {
                    ('W', false) => 604_800,
                    ('D', false) => 86_400,
                    ('H', true) => 3_600,
                    ('M', true) => 60,
                    ('S', true) => 1,
                    _ => return None,
                };
            }
            _ => return None,
        }
    }
    (any && digits.is_empty()).then(|| TimeDelta::seconds(secs))
}

fn kind(summary: &str) -> Option<RoundKind> {
    let s = summary.to_lowercase();
    KINDS
        .iter()
        .find(|(_, words)| words.iter().any(|w| s.contains(w)))
        .map(|(kind, _)| *kind)
}

/// The first link in `text` to a video call.
fn meeting_link(text: &str) -> Option<String> {
    text.match_indices("http").find_map(|(i, _)| {
        let rest = &text[i..];
        if !(rest.starts_with("https://") || rest.starts_with("http://")) {
            return None;
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | ')' | ']'))
            .unwrap_or(rest.len());
        let url = rest[..end].trim_end_matches(['.', ',', ';']);
        MEETING_HOSTS
            .iter()
            .any(|host| url.contains(host))
            .then(|| url.to_owned())
    })
}

/// A description as notes: Google Calendar's joining block and HTML dropped, blank runs closed up, at most
/// [`NOTES_CHARS`] characters.
fn notes(description: &str) -> String {
    let d = description.split("-::~").next().unwrap_or("");
    let d = if looks_like_html(d) {
        html_text(d)
    } else {
        d.to_owned()
    };
    let mut out = String::with_capacity(d.len());
    let mut blank = false;
    for line in d.lines().map(str::trim) {
        if line.is_empty() {
            blank = !out.is_empty();
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank { "\n\n" } else { "\n" });
        }
        out.push_str(line);
        blank = false;
    }
    clip(&out, NOTES_CHARS)
}

fn looks_like_html(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    ["<br", "</", "<p>", "<a "].iter().any(|t| s.contains(t))
}

/// HTML as text: line breaks at `<br>` and block ends, other tags dropped, the common entities decoded.
fn html_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            rest = &rest[open..];
            break;
        };
        let tag = rest[open + 1..open + close].trim().to_ascii_lowercase();
        if tag.starts_with("br")
            || ["/p", "/div", "/li", "/h"]
                .iter()
                .any(|t| tag.starts_with(t))
        {
            out.push('\n');
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// `s` cut to at most `max` characters at a word, with "…".
fn clip(s: &str, max: usize) -> String {
    let Some((cut, _)) = s.char_indices().nth(max) else {
        return s.to_owned();
    };
    let head = &s[..cut];
    let head = head.rfind(char::is_whitespace).map_or(head, |i| &head[..i]);
    let mut out = head.trim_end().to_owned();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use chrono::{Local, NaiveDate, TimeZone};

    use super::{clip, duration, kind, read};
    use crate::dto::RoundKind;

    /// As Google Calendar writes an invite: a `VTIMEZONE`, a `TZID` start, lines folded at 75 octets (here one
    /// mid-URL), escaped commas, a Meet link in its own property and the joining block in the description.
    const GOOGLE: &str = "BEGIN:VCALENDAR\r\n\
PRODID:-//Google Inc//Google Calendar 70.9054//EN\r\n\
VERSION:2.0\r\n\
METHOD:REQUEST\r\n\
BEGIN:VTIMEZONE\r\n\
TZID:America/New_York\r\n\
BEGIN:DAYLIGHT\r\n\
TZOFFSETFROM:-0500\r\n\
TZOFFSETTO:-0400\r\n\
DTSTART:19700308T020000\r\n\
END:DAYLIGHT\r\n\
END:VTIMEZONE\r\n\
BEGIN:VEVENT\r\n\
DTSTART;TZID=America/New_York:20261103T140000\r\n\
DTEND;TZID=America/New_York:20261103T144500\r\n\
DTSTAMP:20261027T150000Z\r\n\
ORGANIZER;CN=\"Shah, Priya\":mailto:priya@globex.com\r\n\
UID:abc123@google.com\r\n\
ATTENDEE;CUTYPE=INDIVIDUAL;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;CN=jan\r\n\
\x20e@example.com;X-NUM-GUESTS=0:mailto:jane@example.com\r\n\
X-GOOGLE-CONFERENCE:https://meet.google.com/abc-defg-hij\r\n\
DESCRIPTION:Technical interview with the platform team\\, about 45 minutes.\r\n\
\x20\\nBring a laptop: we'll pair on a small Go service.\\n\\n-::~:~::~:~:~:~:~:~\r\n\
\x20:~:~:~:~:~:~:~:~::~:~::-\\nJoin with Google Meet: https://meet.google.com/a\r\n\
\x20bc-defg-hij\\n\r\n\
LOCATION:\r\n\
SEQUENCE:0\r\n\
STATUS:CONFIRMED\r\n\
SUMMARY:Globex – Technical Interview (Jane Doe)\r\n\
BEGIN:VALARM\r\n\
ACTION:DISPLAY\r\n\
DESCRIPTION:This is an event reminder\r\n\
TRIGGER:-P0DT0H10M0S\r\n\
END:VALARM\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

    /// As Zoom writes one: UTC times, a `DURATION`, the link as `LOCATION` and in the description.
    const ZOOM: &str = "BEGIN:VCALENDAR\n\
PRODID:-//zoom.us//iCalendar Event//EN\n\
VERSION:2.0\n\
BEGIN:VEVENT\n\
DTSTAMP:20261027T120000Z\n\
DTSTART:20261105T170000Z\n\
DURATION:PT1H\n\
SUMMARY:Recruiter Phone Screen - Initech\n\
UID:20261027T120000Z-123@zoom.us\n\
ORGANIZER;CN=Bill Lumbergh:mailto:bill@initech.com\n\
DESCRIPTION:Bill Lumbergh is inviting you to a scheduled Zoom meeting.\\n\\nJ\n\
\x20oin Zoom Meeting\\nhttps://initech.zoom.us/j/8125551234?pwd=abc\\n\\nMeeting \n\
\x20ID: 812 555 1234\n\
LOCATION:https://initech.zoom.us/j/8125551234?pwd=abc\n\
END:VEVENT\n\
END:VCALENDAR\n";

    #[test]
    fn a_google_invite_reads_in_its_zone() {
        let d = read(GOOGLE).unwrap();
        assert_eq!(d.id, None);
        assert_eq!(d.kind, Some(RoundKind::Technical));
        assert_eq!(d.starts_at.as_deref(), Some("2026-11-03T14:00:00-05:00"));
        assert_eq!(d.minutes, Some(45));
        assert_eq!(d.with, "Shah, Priya");
        assert_eq!(d.place, "https://meet.google.com/abc-defg-hij");
        assert_eq!(
            d.notes,
            "Technical interview with the platform team, about 45 minutes.\nBring a laptop: we'll pair on a \
             small Go service."
        );
    }

    #[test]
    fn a_zoom_invite_reads_in_utc() {
        let d = read(ZOOM).unwrap();
        assert_eq!(d.kind, Some(RoundKind::RecruiterScreen));
        assert_eq!(d.starts_at.as_deref(), Some("2026-11-05T17:00:00Z"));
        assert_eq!(d.minutes, Some(60));
        assert_eq!(d.with, "Bill Lumbergh");
        assert_eq!(d.place, "https://initech.zoom.us/j/8125551234?pwd=abc");
        assert!(d.notes.starts_with(
            "Bill Lumbergh is inviting you to a scheduled Zoom meeting.\n\nJoin Zoom"
        ));
        assert!(d.notes.ends_with("Meeting ID: 812 555 1234"));
    }

    #[test]
    fn floating_and_all_day_times_are_the_macs() {
        let ics = "BEGIN:VEVENT\nDTSTART:20261110T093000\nDTEND:20261110T120000\nSUMMARY:Onsite loop\n\
                   LOCATION:1 Infinite Loop\\, Cupertino\nDESCRIPTION:<p>Ask for <b>Ana</b> at the desk.</p><br>\
                   Badge &amp; ID.\nEND:VEVENT\n";
        let d = read(ics).unwrap();
        let local = Local
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2026, 11, 10)
                    .unwrap()
                    .and_hms_opt(9, 30, 0)
                    .unwrap(),
            )
            .earliest()
            .unwrap();
        assert_eq!(
            d.starts_at.as_deref(),
            Some(
                local
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
                    .as_str()
            )
        );
        assert_eq!((d.kind, d.minutes), (Some(RoundKind::Onsite), Some(150)));
        assert_eq!(d.place, "1 Infinite Loop, Cupertino");
        assert_eq!(d.notes, "Ask for Ana at the desk.\n\nBadge & ID.");

        let all_day = read(
            "BEGIN:VEVENT\nDTSTART;VALUE=DATE:20261112\nDTEND;VALUE=DATE:20261113\nSUMMARY:\
                            Take-home due\nEND:VEVENT",
        )
        .unwrap();
        assert!(
            all_day
                .starts_at
                .unwrap()
                .starts_with("2026-11-12T00:00:00")
        );
        assert_eq!(
            (all_day.kind, all_day.minutes),
            (Some(RoundKind::TakeHome), None)
        );
    }

    #[test]
    fn outlook_zones_and_a_file_without_an_event() {
        let ics = "BEGIN:VEVENT\nDTSTART;TZID=\"W. Europe Standard Time\":20260701T100000\nDURATION:PT30M\n\
                   SUMMARY:Chat\nLOCATION:Microsoft Teams Meeting\nDESCRIPTION:Join: \
                   https://teams.microsoft.com/l/meetup-join/xyz.\nEND:VEVENT";
        let d = read(ics).unwrap();
        assert_eq!(d.starts_at.as_deref(), Some("2026-07-01T10:00:00+02:00"));
        assert_eq!((d.kind, d.minutes), (None, Some(30)));
        assert_eq!(d.place, "https://teams.microsoft.com/l/meetup-join/xyz");
        assert_eq!(read("BEGIN:VCALENDAR\nEND:VCALENDAR\n"), None);
    }

    #[test]
    fn durations_kinds_and_clipping() {
        assert_eq!(duration("PT1H30M").unwrap().num_minutes(), 90);
        assert_eq!(duration("P1DT2H").unwrap().num_minutes(), 1560);
        assert_eq!(duration("-PT15M"), None);
        assert_eq!(duration("PT"), None);
        assert_eq!(kind("Technical phone screen"), Some(RoundKind::Technical));
        assert_eq!(
            kind("Final round with hiring manager"),
            Some(RoundKind::HiringManager)
        );
        assert_eq!(kind("Final round"), Some(RoundKind::Final));
        assert_eq!(kind("Coffee"), None);
        assert_eq!(clip("one two three", 9), "one two…");
        assert_eq!(clip("short", 9), "short");
    }
}
