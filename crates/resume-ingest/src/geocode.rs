//! Where a job is, for the review's map: its location as geocoding queries ([`queries`]), each looked up on
//! OpenStreetMap's Nominatim ([`search`]) under its usage policy: an identifying User-Agent, at most one request a
//! second, results cached by the caller.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::sync::Mutex;
use url::Url;

use crate::{Fetcher, IngestError, check_status};

const HOST: &str = "nominatim.openstreetmap.org";
const SEARCH: &str = "https://nominatim.openstreetmap.org/search";
/// Nominatim's policy: name the app.
const USER_AGENT: &str = concat!(
    "gliide/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/bubb4rd/gliide)"
);
/// Nominatim's policy: at most one request a second.
const GAP: Duration = Duration::from_secs(1);
/// The most queries one location makes.
const MAX_QUERIES: usize = 4;
/// Counties (12) and finer; states (8) and countries (4) are too broad to map a job.
const MIN_RANK: u32 = 12;
/// What Nominatim calls a town or city counts at any rank: New York City and Greater London are ranked as regions
/// (10) for their size, yet they're where a job is.
const SETTLEMENTS: &[&str] = &["city", "town", "village", "municipality", "borough", "hamlet"];

/// A place Nominatim found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Geocoded {
    /// Its own name: "Plano".
    pub name: String,
    /// Where it is: "Texas, United States" (state or nearest region, then country; either may be missing).
    pub region: String,
    pub lat: f64,
    pub lon: f64,
    /// [west, south, east, north], degrees.
    pub bbox: [f64; 4],
    /// Its boundary as GeoJSON geometry (Polygon or MultiPolygon); `None` when Nominatim has only a point.
    pub boundary: Option<String>,
}

// ── Queries ──

/// Words that say how a job is worked, not where.
const WORKPLACE: &[&str] = &[
    "hybrid",
    "onsite",
    "on-site",
    "on site",
    "in-office",
    "in office",
    "office",
    "remote",
    "remote-first",
    "fully remote",
    "100% remote",
];

/// Segments that name no place to map.
const NOWHERE: &[&str] = &[
    "remote",
    "anywhere",
    "worldwide",
    "global",
    "nationwide",
    "flexible",
    "various",
    "various locations",
    "multiple locations",
    "multiple",
    "tbd",
    "n/a",
];

/// Phrases that make a segment no place ("CT - ANY CITY").
const VAGUE: &[&str] = &[
    "any city",
    "any location",
    "multiple locations",
    "various locations",
];

const COUNTRIES: &[&str] = &[
    "us",
    "u.s.",
    "usa",
    "u.s.a.",
    "united states",
    "united states of america",
    "america",
    "canada",
    "mexico",
    "uk",
    "u.k.",
    "united kingdom",
    "great britain",
    "england",
    "ireland",
    "germany",
    "france",
    "spain",
    "portugal",
    "italy",
    "netherlands",
    "poland",
    "india",
    "australia",
    "brazil",
    "japan",
    "singapore",
    "israel",
    "philippines",
    "emea",
    "apac",
    "latam",
    "north america",
    "europe",
];

/// US states (and DC) as code and name.
const STATES: &[(&str, &str)] = &[
    ("AL", "Alabama"),
    ("AK", "Alaska"),
    ("AZ", "Arizona"),
    ("AR", "Arkansas"),
    ("CA", "California"),
    ("CO", "Colorado"),
    ("CT", "Connecticut"),
    ("DE", "Delaware"),
    ("DC", "District of Columbia"),
    ("FL", "Florida"),
    ("GA", "Georgia"),
    ("HI", "Hawaii"),
    ("ID", "Idaho"),
    ("IL", "Illinois"),
    ("IN", "Indiana"),
    ("IA", "Iowa"),
    ("KS", "Kansas"),
    ("KY", "Kentucky"),
    ("LA", "Louisiana"),
    ("ME", "Maine"),
    ("MD", "Maryland"),
    ("MA", "Massachusetts"),
    ("MI", "Michigan"),
    ("MN", "Minnesota"),
    ("MS", "Mississippi"),
    ("MO", "Missouri"),
    ("MT", "Montana"),
    ("NE", "Nebraska"),
    ("NV", "Nevada"),
    ("NH", "New Hampshire"),
    ("NJ", "New Jersey"),
    ("NM", "New Mexico"),
    ("NY", "New York"),
    ("NC", "North Carolina"),
    ("ND", "North Dakota"),
    ("OH", "Ohio"),
    ("OK", "Oklahoma"),
    ("OR", "Oregon"),
    ("PA", "Pennsylvania"),
    ("RI", "Rhode Island"),
    ("SC", "South Carolina"),
    ("SD", "South Dakota"),
    ("TN", "Tennessee"),
    ("TX", "Texas"),
    ("UT", "Utah"),
    ("VT", "Vermont"),
    ("VA", "Virginia"),
    ("WA", "Washington"),
    ("WV", "West Virginia"),
    ("WI", "Wisconsin"),
    ("WY", "Wyoming"),
    ("PR", "Puerto Rico"),
];

fn is_state_code(s: &str) -> bool {
    STATES.iter().any(|(c, _)| c.eq_ignore_ascii_case(s))
}

fn is_state(s: &str) -> bool {
    STATES
        .iter()
        .any(|(c, n)| c.eq_ignore_ascii_case(s) || n.eq_ignore_ascii_case(s))
}

fn is_country(s: &str) -> bool {
    COUNTRIES.iter().any(|c| c.eq_ignore_ascii_case(s))
}

/// Names no place: remote, anywhere, a country, a bare state code. A state's name is left to Nominatim (which finds
/// it too broad to map): `New York` and `Washington` are cities too.
fn is_nowhere(s: &str) -> bool {
    let lower = s.to_lowercase();
    NOWHERE.contains(&lower.as_str())
        || WORKPLACE.contains(&lower.as_str())
        || is_country(s)
        || is_state_code(s)
        || VAGUE.iter().any(|v| lower.contains(v))
}

/// The geocoding queries a job's location names, in order, at most four: `Hybrid - Plano, TX` → `Plano, TX`;
/// `Tucson AZ; Alexandria/Chantilly VA` → `Tucson, AZ`, `Alexandria, VA`; `US-IA-CEDAR RAPIDS-105 ~ …` →
/// `Cedar Rapids, IA`. Remote, a country or a bare state code names nothing.
pub fn queries(location: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for segment in location.split(';') {
        let Some(q) = query(segment) else {
            continue;
        };
        if !out.iter().any(|o| key(o) == key(&q)) {
            out.push(q);
        }
        if out.len() == MAX_QUERIES {
            break;
        }
    }
    out
}

/// One `;`-separated segment as a query, when it names a place.
fn query(segment: &str) -> Option<String> {
    let mut s = segment
        .split(" ~ ")
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    loop {
        let before = s.clone();
        s = strip_trailing_marker(&s);
        s = strip_leading_marker(&s);
        if s == before {
            break;
        }
    }
    if s.is_empty() || is_nowhere(&s) {
        return None;
    }
    if let Some(ats) = ats_code(&s) {
        return Some(ats);
    }
    // `CT - Hartford` → `Hartford, CT`.
    if let Some((state, city)) = s.split_once(" - ")
        && is_state(state.trim())
    {
        let city = city.trim();
        if city.is_empty() || is_nowhere(city) {
            return None;
        }
        return Some(format!("{city}, {}", state.trim()));
    }
    // `Alexandria/Chantilly VA` → `Alexandria VA`; `Raleigh/Durham, NC` → `Raleigh, NC`.
    if let Some((first, rest)) = s.split_once('/') {
        let first = first.trim();
        let state = match rest.rsplit_once(',') {
            Some((_, after)) => after.trim(),
            None => rest.split_whitespace().last().unwrap_or_default(),
        };
        s = if is_state(state) && !first.is_empty() {
            format!("{first}, {state}")
        } else {
            first.to_owned()
        };
    }
    // Parts after the first that name a country (or nothing) add nothing: `Santa Clara, CALIFORNIA, United States`.
    let mut parts: Vec<&str> = s.split(',').map(str::trim).collect();
    while parts.len() > 1
        && parts.last().is_some_and(|p| {
            p.is_empty() || is_country(p) || WORKPLACE.contains(&p.to_lowercase().as_str())
        })
    {
        parts.pop();
    }
    if parts.iter().all(|p| is_nowhere(p)) {
        return None;
    }
    let mut s = parts.join(", ");
    // `Tucson AZ` → `Tucson, AZ`.
    if parts.len() == 1
        && let Some((city, state)) = s.rsplit_once(' ')
        && is_state_code(state)
        && !city.trim().is_empty()
    {
        s = format!("{}, {state}", city.trim());
    }
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    (!s.is_empty()).then_some(s)
}

/// Drops a trailing `(hybrid)`, `(On-site)`, `(US)`: a workplace kind or a country in parentheses.
fn strip_trailing_marker(s: &str) -> String {
    let t = s.trim();
    if let Some(open) = t.rfind('(')
        && t.ends_with(')')
    {
        let inner = t[open + 1..t.len() - 1].trim();
        let lower = inner.to_lowercase();
        if WORKPLACE.contains(&lower.as_str())
            || is_country(inner)
            || NOWHERE.contains(&lower.as_str())
        {
            return t[..open].trim().to_owned();
        }
    }
    t.to_owned()
}

/// Drops a leading `Hybrid - `, `Onsite: `, `USA - `: a workplace kind or a country, then a dash or colon.
fn strip_leading_marker(s: &str) -> String {
    let t = s.trim();
    for sep in [" - ", " – ", " — ", ": ", " | "] {
        if let Some((head, rest)) = t.split_once(sep) {
            let lower = head.trim().to_lowercase();
            if WORKPLACE.contains(&lower.as_str()) || is_country(head.trim()) {
                return rest.trim().to_owned();
            }
        }
    }
    t.to_owned()
}

/// An ATS location code as a query: `US-IA-CEDAR RAPIDS-105` → `Cedar Rapids, IA`; `US-Maryland-Hunt Valley` →
/// `Hunt Valley, Maryland`.
fn ats_code(s: &str) -> Option<String> {
    let mut parts = s.split('-').map(str::trim);
    let country = parts.next()?;
    if !matches!(country.to_ascii_uppercase().as_str(), "US" | "USA") {
        return None;
    }
    let state = parts.next()?;
    let city = parts.next()?;
    if !is_state(state) || city.is_empty() || city.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let city = if city.chars().any(char::is_lowercase) {
        city.to_owned()
    } else {
        title_case(city)
    };
    Some(format!("{city}, {state}"))
}

/// `CEDAR RAPIDS` → `Cedar Rapids`.
fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            chars
                .next()
                .map(|c| {
                    c.to_uppercase()
                        .chain(chars.flat_map(char::to_lowercase))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// A query's cache key: lowercased, whitespace collapsed.
pub fn key(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

// ── Nominatim ──

/// When the last request to Nominatim went out; held across a request, so requests are sequential.
static LAST: Mutex<Option<Instant>> = Mutex::const_new(None);

/// The place `query` names, from Nominatim: `None` when it finds nothing, or only something broader than a county
/// (a state, a country). A query of an area (`Greater Chicago Area`, `San Francisco Bay Area`) that finds nothing is
/// tried once more as its city.
pub async fn search(fetcher: &impl Fetcher, query: &str) -> Result<Option<Geocoded>, IngestError> {
    if let Some(found) = search_once(fetcher, query).await? {
        return Ok(Some(found));
    }
    match city_of_area(query) {
        Some(city) => search_once(fetcher, &city).await,
        None => Ok(None),
    }
}

async fn search_once(fetcher: &impl Fetcher, query: &str) -> Result<Option<Geocoded>, IngestError> {
    let url = search_url(query);
    let mut last = LAST.lock().await;
    if let Some(at) = *last {
        tokio::time::sleep(GAP.saturating_sub(at.elapsed())).await;
    }
    let resp = fetcher.get_as(&url, USER_AGENT).await;
    *last = Some(Instant::now());
    drop(last);
    let resp = resp.map_err(|message| IngestError::Network {
        host: HOST.to_owned(),
        message,
    })?;
    check_status(&resp, HOST, &url)?;
    parse(&resp.body)
}

fn search_url(query: &str) -> String {
    let mut url = Url::parse(SEARCH).expect("static URL");
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("format", "jsonv2")
        .append_pair("limit", "1")
        .append_pair("addressdetails", "1")
        .append_pair("polygon_geojson", "1")
        .append_pair("polygon_threshold", "0.001")
        .append_pair("accept-language", "en");
    url.into()
}

/// `Greater Chicago Area` → `Chicago`, `San Francisco Bay Area` → `San Francisco`: `None` for a query without
/// those words.
fn city_of_area(query: &str) -> Option<String> {
    let words: Vec<&str> = query.split_whitespace().collect();
    let area = |w: &str| {
        matches!(
            w.to_lowercase().trim_end_matches(','),
            "greater" | "area" | "metro" | "metropolitan" | "metroplex"
        )
    };
    if !words.iter().any(|w| area(w)) {
        return None;
    }
    let mut kept: Vec<&str> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let bay_area = w.eq_ignore_ascii_case("bay")
            && words
                .get(i + 1)
                .is_some_and(|n| n.eq_ignore_ascii_case("area"));
        if !area(w) && !bay_area {
            kept.push(w);
        }
    }
    let city = kept.join(" ");
    let city = city.trim_matches(|c: char| c == ',' || c.is_whitespace());
    (!city.is_empty()).then(|| city.to_owned())
}

/// Nominatim's `jsonv2` answer as the place it found: `None` for no result, or one broader than a county.
pub fn parse(body: &str) -> Result<Option<Geocoded>, IngestError> {
    #[derive(Deserialize)]
    struct Hit {
        name: Option<String>,
        display_name: String,
        lat: String,
        lon: String,
        place_rank: u32,
        #[serde(default)]
        addresstype: String,
        #[serde(default)]
        address: Map<String, Value>,
        /// [south, north, west, east].
        boundingbox: [String; 4],
        geojson: Option<Value>,
    }
    let bad = || IngestError::Network {
        host: HOST.to_owned(),
        message: "its answer wasn't the search results it documents".to_owned(),
    };
    let hits: Vec<Hit> = serde_json::from_str(body).map_err(|_| bad())?;
    let Some(hit) = hits.into_iter().next() else {
        return Ok(None);
    };
    if hit.place_rank < MIN_RANK && !SETTLEMENTS.contains(&hit.addresstype.as_str()) {
        return Ok(None);
    }
    let num = |s: &str| s.trim().parse::<f64>().map_err(|_| bad());
    let [south, north, west, east] = &hit.boundingbox;
    let name = hit
        .name
        .filter(|n| !n.trim().is_empty())
        .or_else(|| {
            hit.display_name
                .split(',')
                .next()
                .map(|n| n.trim().to_owned())
        })
        .unwrap_or_default();
    let field = |k: &str| {
        hit.address
            .get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
    };
    let region = ["state", "region", "province", "county"]
        .into_iter()
        .find_map(field)
        .into_iter()
        .chain(field("country"))
        .collect::<Vec<_>>()
        .join(", ");
    let boundary = hit
        .geojson
        .filter(|g| {
            matches!(
                g.get("type").and_then(Value::as_str),
                Some("Polygon" | "MultiPolygon")
            )
        })
        .map(|g| g.to_string());
    Ok(Some(Geocoded {
        name,
        region,
        lat: num(&hit.lat)?,
        lon: num(&hit.lon)?,
        bbox: [num(west)?, num(south)?, num(east)?, num(north)?],
        boundary,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/ingest/nominatim")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    #[test]
    fn queries_of_real_locations() {
        let cases: &[(&str, &[&str])] = &[
            ("Hybrid - Plano, TX", &["Plano, TX"]),
            ("Onsite - Cedar Rapids, IA", &["Cedar Rapids, IA"]),
            ("Houston, TX (onsite)", &["Houston, TX"]),
            ("Fullerton, CA (Onsite)", &["Fullerton, CA"]),
            ("Dallas, TX (On-site)", &["Dallas, TX"]),
            (
                "San Francisco Bay Area (hybrid)",
                &["San Francisco Bay Area"],
            ),
            ("Greater Chicago Area", &["Greater Chicago Area"]),
            ("Remote", &[]),
            ("Remote (US)", &[]),
            ("United States", &[]),
            ("US", &[]),
            ("", &[]),
            (
                "Hybrid - Riverwoods, IL; McLean, VA",
                &["Riverwoods, IL", "McLean, VA"],
            ),
            (
                "Tucson AZ; Alexandria/Chantilly VA; St. Louis MO; MD; CO",
                &["Tucson, AZ", "Alexandria, VA", "St. Louis, MO"],
            ),
            (
                "Santa Clara, CALIFORNIA, United States (hybrid)",
                &["Santa Clara, CALIFORNIA"],
            ),
            (
                "US-IA-CEDAR RAPIDS-105 ~ 400 Collins Rd NE ~ BLDG 105",
                &["Cedar Rapids, IA"],
            ),
            ("US-Maryland-Hunt Valley", &["Hunt Valley, Maryland"]),
            ("USA - Berkeley, MO", &["Berkeley, MO"]),
            ("USA - CT - ANY CITY", &[]),
            ("Dallas; Seattle (hybrid)", &["Dallas", "Seattle"]),
            ("Remote - Austin, TX", &["Austin, TX"]),
            ("Raleigh/Durham, NC", &["Raleigh, NC"]),
            ("Remote, United States", &[]),
            ("TX", &[]),
            ("New York, NY", &["New York, NY"]),
            (
                "Austin, TX; austin,  tx; Boston, MA; Denver, CO; Miami, FL",
                &["Austin, TX", "Boston, MA", "Denver, CO", "Miami, FL"],
            ),
        ];
        for (location, want) in cases {
            assert_eq!(queries(location), *want, "{location:?}");
        }
    }

    #[test]
    fn keys_ignore_case_and_spacing() {
        assert_eq!(key("  Plano,   TX "), "plano, tx");
    }

    #[test]
    fn areas_retry_as_their_city() {
        assert_eq!(
            city_of_area("Greater Chicago Area").as_deref(),
            Some("Chicago")
        );
        assert_eq!(
            city_of_area("San Francisco Bay Area").as_deref(),
            Some("San Francisco")
        );
        assert_eq!(
            city_of_area("Dallas-Fort Worth Metroplex").as_deref(),
            Some("Dallas-Fort Worth")
        );
        assert_eq!(city_of_area("Plano, TX"), None);
    }

    #[test]
    fn a_city_with_its_boundary() {
        let place = parse(&fixture("plano-tx.json")).unwrap().unwrap();
        assert_eq!(place.name, "Plano");
        assert_eq!(place.region, "Texas, United States");
        assert_eq!((place.lat, place.lon), (33.0136764, -96.6925096));
        assert_eq!(
            place.bbox,
            [-96.8599747, 32.9970517, -96.6129244, 33.1252040]
        );
        let boundary: Value = serde_json::from_str(place.boundary.as_deref().unwrap()).unwrap();
        assert_eq!(boundary["type"], "Polygon");
    }

    #[test]
    fn a_city_ranked_as_a_region_still_counts() {
        let place = parse(&fixture("new-york-ny.json")).unwrap().unwrap();
        assert_eq!(place.name, "New York");
        assert_eq!(place.region, "New York, United States");
    }

    #[test]
    fn a_country_or_nothing_is_no_place() {
        assert_eq!(parse(&fixture("united-states.json")).unwrap(), None);
        assert_eq!(parse("[]").unwrap(), None);
        assert!(parse("<html>").is_err());
    }

    #[test]
    fn the_search_url_names_the_query() {
        let url = search_url("Plano, TX");
        assert!(
            url.starts_with(
                "https://nominatim.openstreetmap.org/search?q=Plano%2C+TX&format=jsonv2"
            )
        );
        assert!(url.contains("polygon_geojson=1"));
    }
}
