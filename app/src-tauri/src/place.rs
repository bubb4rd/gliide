//! Where a job is, for the review's map: its location's places, geocoded on OpenStreetMap's Nominatim and cached in
//! the store per query. A place found is kept for good (towns don't move); a query that found nothing is tried again
//! after 14 days; a failed lookup isn't recorded, so the next look tries again.

use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};
use resume_ingest::HttpFetcher;
use resume_ingest::geocode::{self, Geocoded};
use resume_store::PlaceLookup;

use crate::dto::Place;
use crate::error::{AppError, Result};
use crate::state::{App, lock};

/// A query that found nothing is looked up again after this many days.
const RETRY_NONE_DAYS: i64 = 14;

impl From<Geocoded> for Place {
    fn from(g: Geocoded) -> Self {
        Self {
            name: g.name,
            region: g.region,
            lat: g.lat,
            lon: g.lon,
            bbox: g.bbox,
            boundary: g.boundary,
        }
    }
}

/// The places `location` (a job's, as written) names, in its order, each once: empty when it names none (remote, a
/// country, a state) or none was found. Each query comes from the store, else Nominatim (at most one request a
/// second, so a first look at a location naming several places takes a few seconds).
pub async fn places(app: &Arc<App>, location: &str) -> Result<Vec<Place>> {
    let fetcher = HttpFetcher::new();
    let now = Utc::now();
    let mut out: Vec<Place> = Vec::new();
    for query in geocode::queries(location) {
        let key = geocode::key(&query);
        let stored = lock(&app.store).place_lookup(&key)?;
        let place = match stored.and_then(|l| cached(&l, now)) {
            Some(place) => place,
            None => {
                let found = geocode::search(&fetcher, &query).await?.map(Place::from);
                let json = found
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(|e| AppError::other(format!("Couldn't store the place: {e}")))?;
                lock(&app.store).put_place_lookup(&key, json.as_deref())?;
                found
            }
        };
        if let Some(p) = place
            && !out.iter().any(|o| o.lat == p.lat && o.lon == p.lon)
        {
            out.push(p);
        }
    }
    Ok(out)
}

/// A stored lookup still good: the place it found (for good), or nothing (under 14 days old). `None` when it's due
/// again, or its place no longer reads.
fn cached(l: &PlaceLookup, now: DateTime<Utc>) -> Option<Option<Place>> {
    match &l.json {
        Some(json) => serde_json::from_str(json).ok().map(Some),
        None => DateTime::parse_from_rfc3339(&l.tried_at)
            .is_ok_and(|t| now - t.with_timezone(&Utc) < TimeDelta::days(RETRY_NONE_DAYS))
            .then_some(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(json: Option<&str>, tried_at: &str) -> PlaceLookup {
        PlaceLookup {
            json: json.map(str::to_owned),
            tried_at: tried_at.to_owned(),
        }
    }

    #[test]
    fn found_places_keep_and_nothing_found_retries() {
        let now = DateTime::parse_from_rfc3339("2026-10-19T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let plano = Place {
            name: "Plano".into(),
            region: "Texas, United States".into(),
            lat: 33.0,
            lon: -96.7,
            bbox: [-96.9, 33.0, -96.6, 33.1],
            boundary: None,
        };
        let json = serde_json::to_string(&plano).unwrap();
        assert_eq!(
            cached(&lookup(Some(&json), "2020-01-01T00:00:00Z"), now),
            Some(Some(plano))
        );
        assert_eq!(
            cached(&lookup(None, "2026-10-10T00:00:00Z"), now),
            Some(None)
        );
        assert_eq!(cached(&lookup(None, "2026-09-01T00:00:00Z"), now), None);
        assert_eq!(
            cached(&lookup(Some("{"), "2026-10-18T00:00:00Z"), now),
            None
        );
    }
}
