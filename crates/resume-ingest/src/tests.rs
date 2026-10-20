use std::path::PathBuf;

use super::*;
use crate::record::Replay;
use resume_core::salary::{Period, Salary};

fn year(min: u32, max: u32) -> Option<Salary> {
    Salary::new(Some(min * 100), Some(max * 100), "USD", Period::Year)
}

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
    let p = ok_case("greenhouse-fabrikam").await;
    assert_eq!(p.method, Method::Greenhouse);
    assert_eq!(
        p.source.as_deref(),
        Some("https://job-boards.greenhouse.io/fabrikam/jobs/4000000001")
    );
    assert_eq!(
        p.url.as_deref(),
        Some("https://job-boards.greenhouse.io/fabrikam/jobs/4000000001")
    );
    assert_eq!(p.title.as_deref(), Some("Mobile Developer I - US Remote"));
    assert_eq!(p.company.as_deref(), Some("Fabrikam Health"));
    assert_eq!(p.location.as_deref(), Some("Remote"));
    assert!(p.text.starts_with("What is Fabrikam?\n"));
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
async fn ashby_job_page_names_the_company() {
    let p = ok_case("ashby-initech").await;
    assert_eq!(p.method, Method::Ashby);
    assert_eq!(p.title.as_deref(), Some("Entry-Level Software Engineer"));
    assert_eq!(p.company.as_deref(), Some("Initech"));
    // From the page's JSON-LD: the text names only the entry-level figure.
    assert_eq!(p.salary, year(79_800, 91_800));
    assert!(
        p.location
            .as_deref()
            .unwrap()
            .starts_with("Dallas; Seattle; Houston")
    );
    assert!(p.location.as_deref().unwrap().ends_with("(hybrid)"));
    assert_eq!(
        p.url.as_deref(),
        Some("https://jobs.ashbyhq.com/initech/5b1e2c3d-0000-4a00-8000-00000000a001")
    );
    assert!(
        p.text
            .starts_with("Initech is a North American-based, employee-owned")
    );
    assert!(p.text.contains("\n\nYou Can Expect To:\n"));
    assert!(
        p.text
            .contains("\n- Grow your career with the support of a dedicated mentor")
    );
}

#[tokio::test]
async fn ashby_link_with_tracking_query() {
    let p = ok_case("ashby-piedpiper").await;
    assert_eq!(p.method, Method::Ashby);
    assert_eq!(p.title.as_deref(), Some("Associate Salesforce Engineer"));
    assert_eq!(p.company.as_deref(), Some("Pied Piper"));
    assert_eq!(p.location.as_deref(), Some("Remote, United States"));
    assert_eq!(
        p.url.as_deref(),
        Some("https://jobs.ashbyhq.com/piedpiper/5b1e2c3d-0000-4a00-8000-00000000a002")
    );
    assert!(p.text.starts_with("About Pied Piper\n"));
    assert!(has_line(&p.text, "Summary"));
}

#[tokio::test]
async fn ashby_closed_job_is_not_a_posting() {
    let err = case("ashby-closed").await.unwrap_err();
    let gone = "https://jobs.ashbyhq.com/initech/00000000-0000-4000-8000-000000000000";
    assert!(
        matches!(&err, IngestError::NotAPosting { url, reason } if url == gone && reason.contains("may have closed")),
        "{err:?}"
    );
}

#[test]
fn ashby_page_without_app_data_is_read_as_a_page() {
    assert!(boards::ashby("<html><body><main>Jobs</main></body></html>").is_none());
    assert!(boards::ashby("<script>window.__appData = {\"posting\": 1};</script>").is_none());
}

#[tokio::test]
async fn lever_api() {
    let p = ok_case("lever-umbrella").await;
    assert_eq!(p.method, Method::Lever);
    assert_eq!(p.title.as_deref(), Some("Android Engineer - Experience"));
    assert_eq!(p.company, None);
    assert_eq!(p.location.as_deref(), Some("London; Stockholm (hybrid)"));
    assert_eq!(
        p.url.as_deref(),
        Some("https://jobs.lever.co/umbrella/7c2d1e00-0000-4b00-8000-00000000b001")
    );
    assert!(
        p.text
            .starts_with("We design Umbrella Audio’s listening experience")
    );
    assert!(p.text.contains(
        "\n\nWhat You'll Do\n- Develop and maintain mobile client components that capture and report listening"
    ));
    assert!(p.text.contains("Umbrella Audio is an equal opportunity employer."));
}

#[tokio::test]
async fn workday_api() {
    let p = ok_case("workday-cyberdyne").await;
    assert_eq!(p.method, Method::Workday);
    assert_eq!(p.title.as_deref(), Some("Software Engineer, Agent Runtime"));
    // The hiring organization without its internal number (`2100 Cyberdyne USA`).
    assert_eq!(p.company.as_deref(), Some("Cyberdyne USA"));
    assert_eq!(p.location.as_deref(), Some("US, Remote"));
    assert_eq!(
        p.url.as_deref(),
        Some(
            "https://cyberdyne.wd5.myworkdayjobs.com/CyberdyneCareers/job/US-Remote/Software-Engineer--Agent-Runtime_JR1000001"
        )
    );
    assert!(
        p.text
            .starts_with("Are you ready to turn agent research into dependable")
    );
    assert!(
        p.text
            .contains("\n\nWhat You’ll be Doing:\n- Track the cutting edge")
    );
}

#[tokio::test]
async fn smartrecruiters_api() {
    let p = ok_case("smartrecruiters-tyrell").await;
    assert_eq!(p.method, Method::SmartRecruiters);
    assert_eq!(
        p.title.as_deref(),
        Some("Senior Endpoint Systems Engineer - (Hybrid Allen, TX)")
    );
    assert_eq!(p.company.as_deref(), Some("Tyrell Data"));
    assert_eq!(
        p.location.as_deref(),
        Some("Allen, TX, United States (hybrid)")
    );
    assert_eq!(p.salary, year(80_237, 139_077));
    assert!(
        p.text
            .starts_with("Company Description\nTyrell Data is a global data")
    );
    assert!(
        p.text
            .contains("\n\nJob Description\nWe are seeking a proficient M365 Engineer")
    );
    assert!(has_line(&p.text, "Qualifications"));
    assert!(has_line(&p.text, "Additional Information"));
}

#[tokio::test]
async fn workable_api() {
    let p = ok_case("workable-soylent").await;
    assert_eq!(p.method, Method::Workable);
    assert_eq!(
        p.title.as_deref(),
        Some("Open-Source Machine Learning Engineer - US Remote")
    );
    assert_eq!(p.company, None);
    assert_eq!(
        p.location.as_deref(),
        Some("New York, United States (remote)")
    );
    assert_eq!(
        p.url.as_deref(),
        Some("https://apply.workable.com/soylent/j/1A2B3C4D5E/")
    );
    assert!(
        p.text
            .starts_with("At Soylent AI, we're on a mission to make open models easy to use.")
    );
    assert!(
        p.text
            .contains("\n\nRequirements\nPlease provide a cover letter")
    );
}

#[tokio::test]
async fn recruitee_api() {
    let p = ok_case("recruitee-dunmore").await;
    assert_eq!(p.method, Method::Recruitee);
    assert_eq!(p.title.as_deref(), Some("Marketing Operations Specialist"));
    assert_eq!(p.company.as_deref(), Some("Dunmore & Lowe LLC"));
    assert_eq!(
        p.location.as_deref(),
        Some("Lincolnshire, Illinois, United States (hybrid)")
    );
    // Pay comes as strings: `{"min": "80000", "max": "100000", "period": "year"}`.
    assert_eq!(p.salary, year(80_000, 100_000));
    assert!(
        p.text
            .starts_with("Dunmore & Lowe is a Top 150 accounting and advisory firm")
    );
    assert!(p.text.contains(
        "\n\nRequirements\n- Bachelor's degree in Marketing, Business, Communications, or a related field."
    ));
}

#[tokio::test]
async fn linkedin_guest_page() {
    let p = ok_case("linkedin-aperture").await;
    assert_eq!(p.method, Method::Linkedin);
    assert_eq!(
        p.source.as_deref(),
        Some("https://www.linkedin.com/jobs/view/4000000101/")
    );
    assert_eq!(
        p.url.as_deref(),
        Some("https://www.linkedin.com/jobs/view/4000000101/")
    );
    assert_eq!(
        p.title.as_deref(),
        Some("Software Engineer (AI Wearable Tech)")
    );
    assert_eq!(p.company.as_deref(), Some("Aperture Wearables"));
    assert_eq!(p.location.as_deref(), Some("United States"));
    assert!(
        p.text
            .starts_with("Aperture Wearables is hiring a Software Engineer to build quick, dependable")
    );
    assert!(p.text.contains(
        "\n\nWhat You’ll Work On\n- Ship responsive web apps in React, Next.js and TypeScript"
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
            .contains("currentJobId=4000000202")
    );
    assert_eq!(
        p.url.as_deref(),
        Some("https://www.linkedin.com/jobs/view/4000000202/")
    );
    assert_eq!(p.title.as_deref(), Some("Analyst, Business Systems"));
    assert_eq!(p.company.as_deref(), Some("Oscorp Biosciences"));
    assert_eq!(p.location.as_deref(), Some("Springfield, IL"));
    assert!(p.text.chars().count() > 3000);
}

#[tokio::test]
async fn json_ld_job_posting() {
    let p = ok_case("jsonld-vandelay").await;
    assert_eq!(p.method, Method::JsonLd);
    assert_eq!(p.title.as_deref(), Some("Systems & Automation Engineer"));
    assert_eq!(p.company.as_deref(), Some("Vandelay Realty & Development, LLC"));
    // `minValue` and `maxValue` written as strings.
    assert_eq!(p.salary, year(80_000, 105_000));
    assert_eq!(p.location.as_deref(), Some("Oak Ridge, IL"));
    // The page's own canonical link, not the link pasted.
    assert_eq!(
        p.url.as_deref(),
        Some(
            "https://www.vandelay.example/openings/jobs/5000001/IL-Illinois/Oak-Ridge/Systems-Automation-Engineer"
        )
    );
    assert!(p.text.starts_with(
        "Systems & Automation Engineer\n- Do you like writing code that removes busywork for real people?\n"
    ));
    assert!(p.text.contains("\n\nRole Overview\n"));
    assert!(has_line(
        &p.text,
        "- Salary: $80,000 to $105,000 per year, based on experience"
    ));
}

#[tokio::test]
async fn html_main_text_without_chrome() {
    let p = ok_case("html-massive-dynamic").await;
    assert_eq!(p.method, Method::Html);
    assert_eq!(p.title.as_deref(), Some("Software Engineer"));
    assert_eq!(p.company.as_deref(), Some("MASSIVE DYNAMIC"));
    assert_eq!(p.location, None);
    assert_eq!(
        p.url.as_deref(),
        Some("https://careers.massivedynamic.example/en_US/careers/JobDetail/Software-Engineer/66000")
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
    "https://www.linkedin.com/jobs/search-results/?currentJobId=4000000202&keywords=engineer";

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

fn replay(name: &str) -> Replay {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/ingest")
        .join(name);
    Replay::load(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
}

/// A feed job as a search lists it, with only what a read uses.
fn feed_job(slug: &str, source: &str, external_id: &str, url: &str) -> resume_core::feed::FeedJob {
    resume_core::feed::FeedJob {
        origin: resume_core::feed::Origin::Freehire,
        description: None,
        slug: slug.into(),
        title: "Listed title".into(),
        company: "Listed company".into(),
        company_slug: None,
        location: Some("Listed place".into()),
        url: url.into(),
        source: source.into(),
        external_id: Some(external_id.into()),
        skills: vec![],
        posted_at: None,
        found_at: "2026-10-13T20:00:00Z".into(),
        seniority: None,
        employment_type: None,
        work_mode: None,
        years_min: None,
        salary: None,
    }
}

#[tokio::test]
async fn freehire_search_lists_jobs() {
    let query = freehire::Query {
        seniority: vec!["junior".into()],
        employment_type: vec!["full_time".into()],
        category: ["software_engineering", "backend", "frontend", "fullstack"]
            .map(String::from)
            .to_vec(),
        countries: vec!["us".into()],
        fresh_only: true,
        source_exclude: freehire::LINK_SITES.iter().map(|s| s.to_string()).collect(),
        newest_first: true,
        ..Default::default()
    };
    let page = freehire::search(&replay("freehire-search"), &query, 5, 0)
        .await
        .unwrap();
    assert_eq!(page.total, 105);
    assert_eq!(page.jobs.len(), 5);
    let ashby = &page.jobs[0];
    assert_eq!(
        ashby.slug,
        "member-of-technical-staff-enterprise-engineering-i-bay-area-planet-express-k3x9q2ab"
    );
    assert_eq!(
        ashby.external_id.as_deref(),
        Some("planet-express:0b6c1f9e-2d47-4a8b-9c13-5e7f20a4d681")
    );
    assert_eq!(ashby.found_at, "2026-10-13T19:29:18Z");
    assert_eq!(ashby.posted_at.as_deref(), Some("2026-10-13T17:31:45Z"));
    assert_eq!(ashby.company_slug.as_deref(), Some("planet-express"));
    assert!(ashby.skills.iter().any(|s| s == "typescript"));
    assert_eq!(
        ashby.url,
        "https://jobs.ashbyhq.com/planet-express/0b6c1f9e-2d47-4a8b-9c13-5e7f20a4d681"
    );
    let dunder = &page.jobs[1];
    // Only freehire's tracking parameters go; the employer's own stay.
    assert_eq!(
        dunder.url,
        "https://dundermifflin.hrmdirect.example/employment/job-opening.php?req=3840158&req_loc=1511121"
    );
    assert_eq!(dunder.salary, year(90_000, 120_000));
    assert_eq!(dunder.years_min, Some(3));
    assert_eq!(dunder.seniority.as_deref(), Some("junior"));
    assert_eq!(dunder.employment_type.as_deref(), Some("full_time"));
    assert_eq!(dunder.work_mode.as_deref(), Some("remote"));
    assert!(page.jobs.iter().all(|j| !j.url.contains("utm_")));
}

#[tokio::test]
async fn freehire_greenhouse_job_reads_the_board() {
    let job = feed_job(
        "motion-planning-software-engineering-intern-summer-2027-spacely-sprockets-3rxo4u4f",
        "greenhouse",
        "spacely:7000000001",
        "https://spacely.example/careers/7000000001?gh_jid=7000000001",
    );
    // Replay answers only the board API's URL for this job.
    let p = freehire::read_job(&replay("freehire-greenhouse"), &job)
        .await
        .unwrap();
    assert_eq!(p.method, Method::Greenhouse);
    assert_eq!(
        p.title.as_deref(),
        Some("Motion Planning Software Engineering Intern, Summer 2027")
    );
    assert_eq!(p.source.as_deref(), Some(job.page_url().as_str()));
    assert_eq!(p.url.as_deref(), Some(job.url.as_str()));
    assert!(p.text.starts_with("About Spacely Sprockets:\n"));
    assert_clean("freehire-greenhouse", &p.text);

    // The same text, so the same hash, as pasting the board link.
    let pasted = read_with(
        &replay("freehire-greenhouse"),
        "https://job-boards.greenhouse.io/spacely/jobs/7000000001",
    )
    .await
    .unwrap();
    assert_eq!(p.hash, pasted.hash);
}

#[tokio::test]
async fn freehire_workday_job_reads_the_board() {
    let job = feed_job(
        "software-engineer-agent-runtime-cyberdyne",
        "workday",
        "cyberdyne.wd5.myworkdayjobs.com/CyberdyneCareers:/job/US-Remote/Software-Engineer--Agent-Runtime_JR1000001",
        "https://cyberdyne.wd5.myworkdayjobs.com/CyberdyneCareers/job/US-Remote/Software-Engineer--Agent-Runtime_JR1000001",
    );
    // Replay answers only Workday's job URL.
    let p = freehire::read_job(&replay("workday-cyberdyne"), &job)
        .await
        .unwrap();
    assert_eq!(p.method, Method::Workday);
    assert_eq!(p.title.as_deref(), Some("Software Engineer, Agent Runtime"));
    let pasted = ok_case("workday-cyberdyne").await;
    assert_eq!(p.hash, pasted.hash);
}

#[tokio::test]
async fn freehire_other_sources_read_freehires_copy() {
    let job = resume_core::feed::FeedJob {
        title: "Software Engineer".into(),
        company: "Cogswell Cogs".into(),
        location: Some("Seattle, Washington, United States".into()),
        ..feed_job(
            "software-engineer-cogswell-cogs-nw3mel5f",
            "icims",
            "careers.cogswell.example:30535",
            "https://careers.cogswell.example/jobs/30535?lang=en-us",
        )
    };
    let p = freehire::read_job(&replay("freehire-icims"), &job)
        .await
        .unwrap();
    assert_eq!(p.method, Method::Freehire);
    assert_eq!(p.title.as_deref(), Some("Software Engineer"));
    assert_eq!(p.company.as_deref(), Some("Cogswell Cogs"));
    assert_eq!(
        p.location.as_deref(),
        Some("Seattle, Washington, United States")
    );
    assert_eq!(p.url.as_deref(), Some(job.url.as_str()));
    assert!(
        p.text
            .starts_with("Company Overview\nCogswell Cogs builds the parts that keep machines turning.")
    );
    assert_clean("freehire-icims", &p.text);
    assert_eq!(p.hash, resume_core::posting::text_hash(&p.text));
}

#[tokio::test]
async fn freehire_board_ids_that_dont_parse_read_freehires_copy() {
    // A Greenhouse job whose id isn't digits: not guessed into a board request.
    let job = feed_job(
        "software-engineer-intern-ai-sirius-cybernetics-7iwfqjh3",
        "greenhouse",
        "spacely:not-an-id",
        "https://spacely.example/careers/x",
    );
    let p = freehire::read_job(&replay("freehire-detail"), &job)
        .await
        .unwrap();
    assert_eq!(p.method, Method::Freehire);
}

#[test]
fn freehire_logo_urls_encode_the_name() {
    assert_eq!(
        logos::freehire_logo_url("Vehement Capital Partners"),
        "https://logo.freehire.me/Vehement%20Capital%20Partners"
    );
    assert_eq!(
        logos::freehire_logo_url(" Sirius "),
        "https://logo.freehire.me/Sirius"
    );
    assert_eq!(
        logos::freehire_logo_url("Acme/Labs?"),
        "https://logo.freehire.me/Acme%2FLabs%3F"
    );
}

#[tokio::test]
async fn freehire_company_gives_its_website() {
    // Replay answers only `/companies/sirius`.
    let site = logos::company_website(&replay("freehire-company"), "sirius")
        .await
        .unwrap();
    assert_eq!(site.as_deref(), Some("https://www.sirius.example/"));
    // A company freehire doesn't know has none; a failure is an error.
    let none = logos::company_website(&Stub::status(404), "nobody").await;
    assert_eq!(none, Ok(None));
    let err = logos::company_website(&Offline, "sirius").await.unwrap_err();
    assert!(matches!(err, IngestError::Network { .. }), "{err:?}");
}

#[test]
fn icon_links_of_a_recorded_homepage() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ingest/homepage-sirius");
    let r = replay("homepage-sirius").recording;
    let html = std::fs::read_to_string(dir.join(&r.body)).unwrap();
    // The touch icon first, then the icons largest first; the relative `/favicon.ico` is the fallback already.
    assert_eq!(
        logos::icon_links(&html, &r.url),
        [
            "https://www.sirius.example/wp-content/uploads/android-chrome-256x256-250x250.png",
            "https://www.sirius.example/wp-content/uploads/android-chrome-256x256-100x100.png",
            "https://www.sirius.example/favicon.ico",
        ]
    );
}

#[test]
fn icon_links_order_resolve_and_drop_boards() {
    let html = r#"<html><head>
        <link rel="icon" href="/img/fav-16.png" sizes="16x16">
        <link rel="shortcut icon" href="favicon.ico">
        <link rel="icon" href="logo-dark.svg" media="(prefers-color-scheme: dark)">
        <link rel="icon" type="image/svg+xml" href="../brand/logo.svg">
        <link rel="icon" href="//cdn.acme.example/fav-48.png" sizes="32x32 48x48">
        <link rel="apple-touch-icon" href="touch.png">
        <link rel="apple-touch-icon-precomposed" href="touch-180.png" sizes="180x180">
        <link rel="mask-icon" href="pinned.svg">
        <link rel="icon" href="https://boards.greenhouse.io/favicon.png" sizes="64x64">
        <link rel="stylesheet" href="site.css">
    </head><body></body></html>"#;
    assert_eq!(
        logos::icon_links(html, "https://acme.example/en/home/"),
        [
            "https://acme.example/en/home/touch-180.png",
            "https://acme.example/en/home/touch.png",
            "https://acme.example/en/brand/logo.svg",
            "https://cdn.acme.example/fav-48.png",
            "https://acme.example/img/fav-16.png",
            "https://acme.example/en/home/favicon.ico",
            "https://acme.example/en/home/logo-dark.svg",
            "https://acme.example/favicon.ico",
        ]
    );
    // `<base>` moves relative links.
    let based = r#"<head><base href="https://static.acme.example/v2/"><link rel="icon" href="i.png"></head>"#;
    assert_eq!(
        logos::icon_links(based, "https://acme.example/"),
        [
            "https://static.acme.example/v2/i.png",
            "https://acme.example/favicon.ico"
        ]
    );
    // A board's page gives nothing: its icons are the board's.
    assert!(logos::icon_links(html, "https://jobs.lever.co/acme").is_empty());
    assert!(logos::icon_links(html, "https://acme.wd1.myworkdayjobs.com/careers").is_empty());
    assert_eq!(
        logos::icon_links("", "https://clever.co/"),
        ["https://clever.co/favicon.ico"]
    );
}
