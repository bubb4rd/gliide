use std::path::PathBuf;
use std::time::Duration;

use resume_core::salary::{Period, Salary};
use serde_json::json;

use super::*;
use crate::Response;
use crate::record::Replay;

/// Answers the recorded run's POST from its case.
struct Run(Replay);

impl Fetcher for Run {
    async fn get(&self, url: &str) -> Result<Response, String> {
        Err(format!("the actor is POSTed, not GET {url}"))
    }

    async fn post_json(
        &self,
        url: &str,
        _body: &str,
        _timeout: Duration,
    ) -> Result<Response, String> {
        self.0.get(url).await
    }
}

fn run(name: &str) -> Run {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/ingest")
        .join(name);
    Run(Replay::load(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())))
}

fn query() -> Query {
    Query {
        time_range: TimeRange::Hour,
        limit: 10,
        ..Query::default()
    }
}

#[tokio::test]
async fn sample_items_become_feed_jobs() {
    let jobs = pull(&run("fantastic-sample"), "dummy", &query())
        .await
        .unwrap();
    assert_eq!(jobs.len(), 6);
    let glyphic = jobs
        .iter()
        .find(|j| j.slug == "fantastic:2410329455")
        .unwrap();
    assert_eq!(glyphic.origin, Origin::Fantastic);
    assert_eq!(glyphic.title, "Scientist / Senior Scientist, Library Prep");
    assert_eq!(glyphic.company, "Glyphic Biotechnologies");
    assert_eq!(
        glyphic.location.as_deref(),
        Some("Berkeley, California, United States")
    );
    assert_eq!(
        glyphic.url,
        "https://job-boards.greenhouse.io/glyphicbiotechnologies/jobs/4449306009"
    );
    assert_eq!(glyphic.source, "greenhouse");
    assert_eq!(glyphic.external_id, None);
    assert_eq!(glyphic.posted_at.as_deref(), Some("2026-10-13T01:20:46Z"));
    assert_eq!(glyphic.found_at, "2026-10-13T01:23:25.863592Z");
    assert_eq!(glyphic.employment_type.as_deref(), Some("full_time"));
    assert_eq!(glyphic.work_mode.as_deref(), Some("onsite"));
    assert_eq!(glyphic.years_min, Some(5));
    assert_eq!(
        glyphic.salary,
        Some(Salary {
            min_cents: Some(11_172_500),
            max_cents: Some(16_195_000),
            currency: "USD".to_owned(),
            period: Period::Year,
        })
    );
    assert!(
        glyphic
            .skills
            .starts_with(&["molecularbiology".to_owned(), "biochemistry".to_owned()])
    );
    let text = glyphic.description.as_deref().unwrap();
    assert!(text.contains("single-molecule proteome sequencing platform"));
    for bad in ["<p", "</", "&amp;", "&nbsp;", "  ", "\n\n\n"] {
        assert!(!text.contains(bad), "description contains {bad:?}");
    }

    let hourly = jobs
        .iter()
        .find(|j| j.slug == "fantastic:2410322102")
        .unwrap();
    assert_eq!(
        hourly
            .salary
            .as_ref()
            .map(|s| (s.min_cents, s.max_cents, s.period)),
        Some((Some(3_100), Some(3_400), Period::Hour))
    );

    let intern = jobs
        .iter()
        .find(|j| j.slug == "fantastic:2410327682")
        .unwrap();
    assert_eq!(intern.employment_type.as_deref(), Some("internship"));
    assert_eq!(intern.years_min, Some(0));
    assert!(intern.skills.contains(&"cpp".to_owned()));

    let contract = jobs
        .iter()
        .find(|j| j.slug == "fantastic:2410323001")
        .unwrap();
    assert_eq!(contract.employment_type.as_deref(), Some("contract"));
    assert_eq!(contract.work_mode.as_deref(), Some("remote"));
    assert_eq!(contract.location.as_deref(), Some("Mexico"));
    assert!(contract.skills.contains(&"nodejs".to_owned()));

    let no_place = jobs
        .iter()
        .find(|j| j.slug == "fantastic:2410329681")
        .unwrap();
    assert_eq!(no_place.location, None);
    assert_eq!(no_place.years_min, Some(10));

    let workday = jobs
        .iter()
        .find(|j| j.slug == "fantastic:2410353075")
        .unwrap();
    assert_eq!(workday.source, "workday");
    assert_eq!(
        workday.location.as_deref(),
        Some("Camperdown, Victoria, Australia")
    );
    assert_eq!(workday.posted_at.as_deref(), Some("2026-10-13T00:00:00Z"));
    assert_eq!(
        workday.salary.as_ref().map(|s| s.currency.as_str()),
        Some("AUD")
    );
}

#[tokio::test]
async fn a_refused_token_says_to_check_it() {
    let err = pull(&run("fantastic-bad-token"), "dummy", &query())
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Apify refused the token; check it in Settings → Feed"
    );
}

#[test]
fn a_full_query_is_the_actor_input() {
    let q = Query {
        titles: vec!["Software Engineer".into(), "Backend".into()],
        locations: vec!["United States".into()],
        time_range: TimeRange::Week,
        experience: vec!["0-2".into(), "2-5".into()],
        employment: vec!["FULL_TIME".into(), "INTERN".into()],
        visa_sponsorship: true,
        limit: 200,
    };
    assert_eq!(
        q.input(),
        json!({
            "timeRange": "7d",
            "limit": 200,
            "titleSearch": ["Software Engineer", "Backend"],
            "locationSearch": ["United States"],
            "aiExperienceLevelFilter": ["0-2", "2-5"],
            "aiEmploymentTypeFilter": ["FULL_TIME", "INTERN"],
            "aiVisaSponsorshipFilter": true,
            "descriptionType": "text",
            "includeCompanyDetails": false
        })
    );
    assert_eq!(
        query().input(),
        json!({ "timeRange": "1h", "limit": 10, "descriptionType": "text", "includeCompanyDetails": false })
    );
}
