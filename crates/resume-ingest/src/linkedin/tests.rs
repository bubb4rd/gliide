use std::path::PathBuf;

use super::*;
use crate::record::Replay;

const NOW: &str = "2026-10-14T02:36:00Z";

fn replay(name: &str) -> Replay {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/ingest")
        .join(name);
    Replay::load(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
}

fn recorded_search() -> Search {
    Search {
        keywords: "software engineer".to_owned(),
        location: Some("United States".to_owned()),
        posted_within_secs: Some(86_400),
        ..Search::default()
    }
}

/// No markup or entities, whitespace collapsed, trimmed.
fn assert_clean(field: &str) {
    for bad in ["<", "&amp;", "&#", "\u{a0}", "  ", "\n", "\t"] {
        assert!(!field.contains(bad), "{field:?} contains {bad:?}");
    }
    assert_eq!(field, field.trim());
}

#[tokio::test]
async fn cards_of_a_real_search_page() {
    let jobs = search(&replay("linkedin-guest-search"), &recorded_search(), 0, NOW)
        .await
        .unwrap();
    assert_eq!(jobs.len(), PAGE as usize);
    assert_eq!(
        jobs[0],
        FeedJob {
            origin: Origin::Linkedin,
            slug: "linkedin:4478772592".to_owned(),
            title: "Java Software Engineer III - Debit Technology".to_owned(),
            company: "JPMorganChase".to_owned(),
            company_slug: None,
            location: Some("Atlanta, GA".to_owned()),
            url: "https://www.linkedin.com/jobs/view/4478772592/".to_owned(),
            source: "linkedin".to_owned(),
            external_id: Some("4478772592".to_owned()),
            skills: Vec::new(),
            posted_at: Some("2026-10-13T00:00:00Z".to_owned()),
            found_at: NOW.to_owned(),
            seniority: None,
            employment_type: None,
            work_mode: None,
            years_min: None,
            salary: None,
            description: None,
        }
    );
    for job in &jobs {
        let id = job.slug.strip_prefix("linkedin:").unwrap();
        assert!(id.bytes().all(|b| b.is_ascii_digit()), "{job:?}");
        assert_eq!(job.url, format!("https://www.linkedin.com/jobs/view/{id}/"));
        let location = job.location.as_deref().unwrap();
        for field in [job.title.as_str(), job.company.as_str(), location] {
            assert_clean(field);
        }
        assert!(job.posted_at.as_deref().unwrap().ends_with("T00:00:00Z"));
    }
}

#[test]
fn url_with_every_filter() {
    let s = Search {
        keywords: " C++ & Rust engineer ".to_owned(),
        location: Some("New York, United States".to_owned()),
        posted_within_secs: Some(3600),
        experience: vec![1, 2],
        job_types: vec!['F', 'C'],
    };
    assert_eq!(
        s.url(20),
        "https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search?keywords=C%2B%2B+%26+Rust+engineer\
         &location=New+York%2C+United+States&f_TPR=r3600&f_E=1%2C2&f_JT=F%2CC&start=20"
    );
}

#[test]
fn a_card_salary_line_is_its_pay() {
    let html = r#"<li><div class="base-search-card" data-entity-urn="urn:li:jobPosting:1">
        <h3 class="base-search-card__title">Engineer</h3>
        <h4 class="base-search-card__subtitle"><a>Acme</a></h4>
        <span class="job-search-card__salary-info">
          $120,000.00 - $150,000.00
        </span></div></li>"#;
    let job = &cards(html, NOW)[0];
    assert_eq!(
        job.salary,
        Salary::new(
            Some(12_000_000),
            Some(15_000_000),
            "USD",
            salary::Period::Year
        )
    );
    assert_eq!(job.location, None);
}

#[tokio::test]
async fn a_page_past_the_end_is_empty() {
    let end = replay("linkedin-guest-search-end");
    let jobs = search(&end, &recorded_search(), 1000, NOW).await.unwrap();
    assert!(jobs.is_empty());
}

/// Answers every GET with `status` and no body.
struct Status(u16);

impl Fetcher for Status {
    async fn get(&self, url: &str) -> Result<crate::Response, String> {
        Ok(crate::Response {
            url: url.to_owned(),
            status: self.0,
            content_type: None,
            retry_after: None,
            body: String::new(),
        })
    }
}

#[tokio::test]
async fn refusals_end_the_search() {
    let err = search(
        &replay("linkedin-guest-search-429"),
        &recorded_search(),
        0,
        NOW,
    )
    .await
    .unwrap_err();
    assert_eq!(
        err,
        IngestError::RateLimited {
            host: "linkedin.com".to_owned(),
            retry_after: None
        }
    );
    let err = search(&Status(999), &recorded_search(), 0, NOW)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        IngestError::Blocked {
            host: "linkedin.com".to_owned(),
            status: 999
        }
    );
}
