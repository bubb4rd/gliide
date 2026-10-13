use std::path::PathBuf;

use super::*;
use crate::record::Replay;

/// Reads a recorded case from `fixtures/ingest/<name>` through its own input.
async fn case(name: &str) -> Result<Posting, IngestError> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/ingest")
        .join(name);
    let replay = Replay::load(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let input = replay.recording.input.clone();
    read_with(&replay, &input).await
}

async fn ok_case(name: &str) -> Posting {
    let p = case(name).await.unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_clean(name, &p.text);
    assert_eq!(p.hash, resume_core::posting::text_hash(&p.text));
    p
}

/// Text quality every source must meet: no markup or entities left, whitespace collapsed, at most one blank line.
fn assert_clean(name: &str, text: &str) {
    for bad in [
        "<p", "<li", "<br", "<div", "</", "&amp;", "&nbsp;", "&lt;", "&#", "\u{a0}", "  ", "\t",
        "\n\n\n",
    ] {
        assert!(!text.contains(bad), "{name}: text contains {bad:?}");
    }
    for line in text.lines() {
        assert_eq!(line, line.trim_end(), "{name}: trailing space in {line:?}");
    }
    assert_eq!(text, text.trim(), "{name}: text isn't trimmed");
}

fn has_line(text: &str, line: &str) -> bool {
    text.lines().any(|l| l == line)
}

#[tokio::test]
async fn greenhouse_api() {
    let p = ok_case("greenhouse-perfectserve").await;
    assert_eq!(p.method, Method::Greenhouse);
    assert_eq!(
        p.source.as_deref(),
        Some("https://job-boards.greenhouse.io/perfectserve/jobs/6188374004")
    );
    assert_eq!(
        p.url.as_deref(),
        Some("https://job-boards.greenhouse.io/perfectserve/jobs/6188374004")
    );
    assert_eq!(p.title.as_deref(), Some("Mobile Developer I - US Remote"));
    assert_eq!(p.company.as_deref(), Some("PerfectServe"));
    assert_eq!(p.location.as_deref(), Some("Remote"));
    assert!(p.text.starts_with("What is PerfectServe?\n"));
    assert!(p.text.contains(
        "\n\nResponsibilities\n- Develop and maintain cross-platform mobile applications using React Native"
    ));
    assert!(has_line(&p.text, "- Health IT experience (a plus)"));
    assert!(
        p.text
            .contains("We offer a salary range of $75,000–$100,000 USD per year")
    );
}

#[tokio::test]
async fn ashby_api_matches_the_job_on_the_board() {
    let p = ok_case("ashby-pariveda").await;
    assert_eq!(p.method, Method::Ashby);
    assert_eq!(p.title.as_deref(), Some("Entry-Level Software Engineer"));
    assert_eq!(p.company, None);
    assert!(
        p.location
            .as_deref()
            .unwrap()
            .starts_with("Dallas; Seattle; Houston")
    );
    assert!(p.location.as_deref().unwrap().ends_with("(hybrid)"));
    assert_eq!(
        p.url.as_deref(),
        Some("https://jobs.ashbyhq.com/pariveda/cc4fc0be-c414-4aba-a15d-64daa03476a0")
    );
    assert!(
        p.text
            .starts_with("Pariveda is a North American-based, employee-owned")
    );
    assert!(p.text.contains("\n\nYou Can Expect To:\n"));
    assert!(
        p.text
            .contains("\n- Grow your career with the support of a dedicated mentor")
    );
}

#[tokio::test]
async fn ashby_job_missing_from_the_board_is_not_a_posting() {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ingest/ashby-pariveda");
    let replay = Replay::load(&dir).unwrap();
    let gone = "https://jobs.ashbyhq.com/pariveda/00000000-0000-4000-8000-000000000000";
    let err = read_with(&replay, gone).await.unwrap_err();
    assert!(
        matches!(&err, IngestError::NotAPosting { url, reason } if url == gone && reason.contains("may have closed")),
        "{err:?}"
    );
}

#[tokio::test]
async fn lever_api() {
    let p = ok_case("lever-spotify").await;
    assert_eq!(p.method, Method::Lever);
    assert_eq!(p.title.as_deref(), Some("Android Engineer - Experience"));
    assert_eq!(p.company, None);
    assert_eq!(p.location.as_deref(), Some("London; Stockholm (hybrid)"));
    assert_eq!(
        p.url.as_deref(),
        Some("https://jobs.lever.co/spotify/2193db3f-77c5-43b8-b030-8f92c9882bf1")
    );
    assert!(
        p.text
            .starts_with("We design Spotify’s consumer experience")
    );
    assert!(p.text.contains(
        "\n\nWhat You'll Do\n- Develop and maintain mobile client components that capture and report listening"
    ));
    assert!(p.text.contains("Spotify is an equal opportunity employer."));
}

#[tokio::test]
async fn linkedin_guest_page() {
    let p = ok_case("linkedin-vilo").await;
    assert_eq!(p.method, Method::Linkedin);
    assert_eq!(
        p.source.as_deref(),
        Some("https://www.linkedin.com/jobs/view/4446030579/")
    );
    assert_eq!(
        p.url.as_deref(),
        Some("https://www.linkedin.com/jobs/view/4446030579/")
    );
    assert_eq!(
        p.title.as_deref(),
        Some("Software Engineer (AI Wearable Tech)")
    );
    assert_eq!(p.company.as_deref(), Some("Vilo"));
    assert_eq!(p.location.as_deref(), Some("United States"));
    assert!(
        p.text
            .starts_with("We are looking for a Software Engineer to build fast, reliable")
    );
    assert!(p.text.contains(
        "\n\nWhat You’ll Work On\n- Build and maintain responsive web applications using React, Next.js"
    ));
    assert!(p.text.ends_with(
        "\n\nSeniority level: Entry level\nEmployment type: Full-time\nIndustries: Computers and Electronics Manufacturing"
    ));
}

#[tokio::test]
async fn linkedin_search_link_reads_the_job() {
    let p = ok_case("linkedin-search").await;
    assert_eq!(p.method, Method::Linkedin);
    assert!(
        p.source
            .as_deref()
            .unwrap()
            .contains("currentJobId=4475737628")
    );
    assert_eq!(
        p.url.as_deref(),
        Some("https://www.linkedin.com/jobs/view/4475737628/")
    );
    assert_eq!(p.title.as_deref(), Some("Analyst, Business Systems"));
    assert_eq!(p.company.as_deref(), Some("BioSpace"));
    assert_eq!(p.location.as_deref(), Some("North Chicago, IL"));
    assert!(p.text.chars().count() > 3000);
}

#[tokio::test]
async fn json_ld_job_posting() {
    let p = ok_case("jsonld-gcrealty").await;
    assert_eq!(p.method, Method::JsonLd);
    assert_eq!(p.title.as_deref(), Some("Systems & Automation Engineer"));
    assert_eq!(p.company.as_deref(), Some("GC Realty & Development, LLC"));
    assert_eq!(p.location.as_deref(), Some("Roselle, IL"));
    // The page's own canonical link, not the link pasted.
    assert_eq!(
        p.url.as_deref(),
        Some(
            "https://www.applicantpro.com/openings/gcrealtyinc/jobs/4234604/IL-Illinois/Roselle/Systems-Automation-Engineer"
        )
    );
    assert!(p.text.starts_with(
        "Systems & Automation Engineer\n- Can you write code that solves real business problems?\n"
    ));
    assert!(p.text.contains("\n\nRole Overview\n"));
    assert!(has_line(
        &p.text,
        "- Salary: $80,000 to $105,000 per year, based on experience"
    ));
}

#[tokio::test]
async fn html_main_text_without_chrome() {
    let p = ok_case("html-mantech").await;
    assert_eq!(p.method, Method::Html);
    assert_eq!(p.title.as_deref(), Some("Software Engineer"));
    assert_eq!(p.company.as_deref(), Some("MANTECH"));
    assert_eq!(p.location, None);
    assert_eq!(
        p.url.as_deref(),
        Some("https://careers.mantech.com/en_US/careers/JobDetail/Software-Engineer/66000")
    );
    assert!(
        p.text
            .contains("\n\nClearance Requirements:\n- Active/current TS/SCI with Polygraph.\n")
    );
    assert!(
        p.text
            .contains("\n\nMinimum Qualifications:\n- Bachelor’s Degree in Computer Science")
    );
    for chrome in [
        "Open Positions",
        "Skip to content",
        "All rights reserved",
        "Privacy Policy",
    ] {
        assert!(
            !p.text.contains(chrome),
            "page chrome {chrome:?} in the text"
        );
    }
}

#[tokio::test]
async fn pasted_text() {
    let pasted =
        "  Software Engineer, ROS\r\n\r\n\r\nRequirements:\r\n* 3+ years   of C++\u{a0}\r\n";
    let p = read_with(&Unreachable, pasted).await.unwrap();
    assert_eq!(
        p,
        Posting::pasted("Software Engineer, ROS\n\nRequirements:\n* 3+ years of C++".into())
    );
    assert_eq!(p.method, Method::Pasted);
    assert_eq!(p.source, None);

    // One line of text with a URL in it is text, not a link.
    let p = read_with(&Unreachable, "Apply at https://example.com/jobs/1 today")
        .await
        .unwrap();
    assert_eq!(p.method, Method::Pasted);

    assert_eq!(
        read_with(&Unreachable, " \n\t ").await,
        Err(IngestError::Empty)
    );
}

/// A fetcher for reads that must not fetch.
struct Unreachable;

impl Fetcher for Unreachable {
    async fn get(&self, url: &str) -> Result<Response, String> {
        panic!("pasted text fetched {url}")
    }
}

/// Answers every request with one status, `Retry-After` and body.
struct Stub {
    status: u16,
    retry_after: Option<&'static str>,
    content_type: &'static str,
    body: &'static str,
}

impl Stub {
    fn status(status: u16) -> Self {
        Self {
            status,
            retry_after: None,
            content_type: "text/html; charset=utf-8",
            body: "",
        }
    }
}

impl Fetcher for Stub {
    async fn get(&self, url: &str) -> Result<Response, String> {
        Ok(Response {
            url: url.to_owned(),
            status: self.status,
            content_type: Some(self.content_type.to_owned()),
            retry_after: self.retry_after.map(str::to_owned),
            body: self.body.to_owned(),
        })
    }
}

struct Offline;

impl Fetcher for Offline {
    async fn get(&self, _: &str) -> Result<Response, String> {
        Err("dns error: failed to lookup address information".to_owned())
    }
}

const SEARCH: &str =
    "https://www.linkedin.com/jobs/search-results/?currentJobId=4475737628&keywords=engineer";

#[tokio::test]
async fn blocked_sites_say_to_paste() {
    let indeed = "https://www.indeed.com/viewjob?jk=0123456789abcdef";
    let err = read_with(&Stub::status(401), indeed).await.unwrap_err();
    assert_eq!(
        err,
        IngestError::Blocked {
            host: "indeed.com".into(),
            status: 401
        }
    );
    assert_eq!(
        err.to_string(),
        "indeed.com refuses automated requests (HTTP 401); copy the posting's text and paste it instead"
    );
    let err = read_with(&Stub::status(403), "https://example.com/careers/1")
        .await
        .unwrap_err();
    assert!(matches!(err, IngestError::Blocked { status: 403, .. }));
    let err = read_with(&Stub::status(999), SEARCH).await.unwrap_err();
    assert!(matches!(err, IngestError::Blocked { status: 999, .. }));
}

#[tokio::test]
async fn rate_limited_with_and_without_retry_after() {
    let stub = Stub {
        retry_after: Some("90"),
        ..Stub::status(429)
    };
    let err = read_with(&stub, SEARCH).await.unwrap_err();
    assert_eq!(
        err,
        IngestError::RateLimited {
            host: "linkedin.com".into(),
            retry_after: Some(Duration::from_secs(90))
        }
    );
    assert_eq!(
        err.to_string(),
        "linkedin.com is limiting requests (HTTP 429), retry in 90 s; copy the posting's text and paste it instead"
    );
    let err = read_with(&Stub::status(429), SEARCH).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "linkedin.com is limiting requests (HTTP 429); copy the posting's text and paste it instead"
    );
}

#[test]
fn retry_after_forms() {
    let now = httpdate::parse_http_date("Mon, 12 Oct 2026 20:00:00 GMT").unwrap();
    assert_eq!(retry_after("120", now), Some(Duration::from_secs(120)));
    assert_eq!(
        retry_after("Mon, 12 Oct 2026 20:05:00 GMT", now),
        Some(Duration::from_secs(300))
    );
    assert_eq!(
        retry_after("Mon, 12 Oct 2026 19:00:00 GMT", now),
        Some(Duration::ZERO)
    );
    assert_eq!(retry_after("soon", now), None);
    assert_eq!(
        retry_hint(Some(Duration::from_secs(600))),
        ", retry in 10 min"
    );
}

#[tokio::test]
async fn pages_drawn_by_javascript_have_too_little_text() {
    let stub = Stub {
        body: "<html><body><nav>Jobs Login</nav><main><div id=app>Loading…</div></main></body></html>",
        ..Stub::status(200)
    };
    let err = read_with(&stub, "https://careers.example.com/job/42")
        .await
        .unwrap_err();
    assert_eq!(
        err,
        IngestError::TooLittleText {
            host: "careers.example.com".into(),
            chars: "Loading…".chars().count()
        }
    );
    assert!(err.to_string().contains("paste it instead"));
}

#[tokio::test]
async fn other_failures() {
    let gone = read_with(
        &Stub::status(404),
        "https://jobs.lever.co/acme/2193db3f-77c5-43b8-b030-8f92c9882bf1",
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&gone, IngestError::NotAPosting { url, .. } if url == "https://jobs.lever.co/acme/2193db3f-77c5-43b8-b030-8f92c9882bf1"),
        "{gone:?}"
    );
    let pdf = Stub {
        content_type: "application/pdf",
        body: "%PDF-1.7",
        ..Stub::status(200)
    };
    let err = read_with(&pdf, "https://example.com/job.pdf")
        .await
        .unwrap_err();
    assert!(matches!(err, IngestError::NotAPosting { .. }), "{err:?}");
    let not_json = Stub {
        body: "<html>maintenance</html>",
        ..Stub::status(200)
    };
    let err = read_with(&not_json, "https://job-boards.greenhouse.io/acme/jobs/1")
        .await
        .unwrap_err();
    assert!(matches!(err, IngestError::NotAPosting { .. }), "{err:?}");
    let err = read_with(&Stub::status(503), "https://example.com/job")
        .await
        .unwrap_err();
    assert!(matches!(err, IngestError::Network { .. }), "{err:?}");
    let err = read_with(&Offline, "example.com/job").await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "Couldn't reach example.com: dns error: failed to lookup address information"
    );
}
