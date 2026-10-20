use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;
use crate::Response;
use crate::record::Replay;

const NOW: &str = "2026-10-13T20:00:00Z";

/// Answers from recorded cases: a GET by its URL, a Workday POST by the `offset` in its body.
struct Cases(Vec<(Option<usize>, Replay)>);

impl Fetcher for Cases {
    async fn get(&self, url: &str) -> Result<Response, String> {
        match self.0.iter().find(|(_, r)| r.recording.request == url) {
            Some((_, r)) => r.get(url).await,
            None => Err(format!("nothing recorded for {url}")),
        }
    }

    async fn post_json(
        &self,
        url: &str,
        body: &str,
        _timeout: Duration,
    ) -> Result<Response, String> {
        let offset = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|v| v["offset"].as_u64())
            .map(|o| o as usize);
        match self
            .0
            .iter()
            .find(|(o, _)| *o == offset && offset.is_some())
        {
            Some((_, r)) => r.get(url).await,
            None => Err(format!("nothing recorded for POST {url} {body}")),
        }
    }
}

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/ingest")
        .join(name)
}

fn load(dir: &Path) -> Replay {
    Replay::load(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
}

/// A case, or a case of one recorded response per page offset (`{case}/{offset}/`).
fn cases(name: &str) -> Cases {
    let d = dir(name);
    if d.join("request.json").exists() {
        return Cases(vec![(None, load(&d))]);
    }
    let mut pages: Vec<(Option<usize>, Replay)> = std::fs::read_dir(&d)
        .unwrap_or_else(|e| panic!("{}: {e}", d.display()))
        .map(|e| e.unwrap().path())
        .map(|p| {
            let offset = p.file_name().unwrap().to_str().unwrap().parse().unwrap();
            (Some(offset), load(&p))
        })
        .collect();
    pages.sort_by_key(|(o, _)| *o);
    Cases(pages)
}

/// Lists the case's board (named by its recorded input) and checks every job's shared fields.
async fn listed(name: &str) -> (Board, Vec<FeedJob>) {
    let cases = cases(name);
    let input = &cases.0[0].1.recording.input;
    let board = Board::parse(input).unwrap_or_else(|| panic!("{input} isn't a board"));
    let jobs = list(&cases, &board, NOW).await.unwrap();
    for job in &jobs {
        assert_eq!(job.origin, Origin::Watchlist);
        assert_eq!(job.source, board.source());
        assert_eq!(job.found_at, NOW);
        assert!(!job.title.is_empty(), "{job:?}");
        let id = job.slug.rsplit(':').next().unwrap();
        assert_eq!(
            job.slug,
            format!("{}:{}", board.source(), job.external_id.as_deref().unwrap())
        );
        assert!(job.url.contains(id), "{job:?}");
        assert!(
            !matches!(
                crate::link::Link::parse(&job.url),
                None | Some(crate::link::Link::Page { .. })
            ),
            "{} isn't read as a board job",
            job.url
        );
    }
    let mut dates: Vec<&str> = jobs.iter().map_while(|j| j.posted_at.as_deref()).collect();
    let n = dates.len();
    dates.sort_by(|a, b| b.cmp(a));
    assert!(
        jobs[..n]
            .iter()
            .zip(&dates)
            .all(|(j, d)| j.posted_at.as_deref() == Some(*d))
    );
    assert!(jobs[n..].iter().all(|j| j.posted_at.is_none()));
    (board, jobs)
}

#[test]
fn parses_board_links() {
    let gh = |b: &str| Some(Board::Greenhouse { board: b.into() });
    assert_eq!(
        Board::parse("https://job-boards.greenhouse.io/fabrikam"),
        gh("fabrikam")
    );
    assert_eq!(
        Board::parse("https://job-boards.greenhouse.io/fabrikam/jobs/4810100004?gh_src=x"),
        gh("fabrikam")
    );
    assert_eq!(
        Board::parse("boards.greenhouse.io/vehement"),
        gh("vehement")
    );
    assert_eq!(
        Board::parse("https://boards.greenhouse.io/embed/job_board?for=vehement"),
        gh("vehement")
    );
    assert_eq!(
        Board::parse("https://jobs.lever.co/umbrella/5d0c7e2a-3b41-4f9a-9e6d-1a2b3c4d5e6f"),
        Some(Board::Lever {
            company: "umbrella".into(),
            eu: false
        })
    );
    assert_eq!(
        Board::parse("jobs.eu.lever.co/acme"),
        Some(Board::Lever {
            company: "acme".into(),
            eu: true
        })
    );
    assert_eq!(
        Board::parse("https://jobs.ashbyhq.com/initech/8f3e2d1c-6b5a-4c9d-8e7f-0a1b2c3d4e5f"),
        Some(Board::Ashby {
            org: "initech".into()
        })
    );
    let cyberdyne = Some(Board::Workday {
        host: "cyberdyne.wd5.myworkdayjobs.com".into(),
        tenant: "cyberdyne".into(),
        site: "CyberdyneCareers".into(),
    });
    assert_eq!(
        Board::parse("cyberdyne.wd5.myworkdayjobs.com/en-US/CyberdyneCareers"),
        cyberdyne
    );
    assert_eq!(
        Board::parse("https://cyberdyne.wd5.myworkdayjobs.com/CyberdyneCareers"),
        cyberdyne
    );
    assert_eq!(
        Board::parse(
            "https://cyberdyne.wd5.myworkdayjobs.com/en-US/CyberdyneCareers/job/US-CA-Sunnyvale/Engineer_JR2000001"
        ),
        cyberdyne
    );
    assert_eq!(
        Board::parse("https://jobs.smartrecruiters.com/SpacelySprockets"),
        Some(Board::SmartRecruiters {
            company: "SpacelySprockets".into()
        })
    );
    assert_eq!(
        Board::parse(
            "https://jobs.smartrecruiters.com/SterlingCooper/6100000001022686-enterprise-account-executive"
        ),
        Some(Board::SmartRecruiters {
            company: "SterlingCooper".into()
        })
    );
    assert_eq!(
        Board::parse("https://apply.workable.com/soylent/"),
        Some(Board::Workable {
            account: "soylent".into()
        })
    );
    assert_eq!(
        Board::parse("https://apply.workable.com/soylent/j/3C7A91E0B4/"),
        Some(Board::Workable {
            account: "soylent".into()
        })
    );
    assert_eq!(
        Board::parse("gringotts.recruitee.com"),
        Some(Board::Recruitee {
            company: "gringotts".into()
        })
    );
    assert_eq!(
        Board::parse("https://gringotts.recruitee.com/o/senior-product-manager-vaults-1"),
        Some(Board::Recruitee {
            company: "gringotts".into()
        })
    );

    assert_eq!(
        Board::parse("https://www.linkedin.com/jobs/view/4000000001/"),
        None
    );
    assert_eq!(
        Board::parse("https://apply.workable.com/j/3C7A91E0B4"),
        None
    );
    assert_eq!(Board::parse("https://careers.example.com/jobs"), None);
}

#[test]
fn board_urls_read_back() {
    for input in [
        "boards.greenhouse.io/vehement",
        "jobs.eu.lever.co/acme/5d0c7e2a-3b41-4f9a-9e6d-1a2b3c4d5e6f",
        "jobs.ashbyhq.com/initech",
        "cyberdyne.wd5.myworkdayjobs.com/en-US/CyberdyneCareers",
        "jobs.smartrecruiters.com/SterlingCooper",
        "apply.workable.com/soylent",
        "gringotts.recruitee.com",
    ] {
        let board = Board::parse(input).unwrap();
        assert_eq!(Board::parse(&board.url()), Some(board.clone()), "{input}");
    }
    let board = Board::parse("cyberdyne.wd5.myworkdayjobs.com/en-US/CyberdyneCareers").unwrap();
    assert_eq!(
        board.url(),
        "https://cyberdyne.wd5.myworkdayjobs.com/CyberdyneCareers"
    );
    assert_eq!(board.name(), "cyberdyne");
}

#[tokio::test]
async fn lists_greenhouse() {
    let (_, jobs) = listed("watch-greenhouse").await;
    assert_eq!(jobs.len(), 10);
    let j = &jobs[0];
    assert_eq!(j.slug, "greenhouse:fabrikam:4810357004");
    assert_eq!(
        j.url,
        "https://job-boards.greenhouse.io/fabrikam/jobs/4810357004"
    );
    assert_eq!(j.title, "Implementation Consultant - US Remote");
    assert_eq!(j.company, "Fabrikam Health");
    assert_eq!(j.posted_at.as_deref(), Some("2026-10-08T16:57:18Z"));
}

#[tokio::test]
async fn lists_lever() {
    let (_, jobs) = listed("watch-lever").await;
    assert_eq!(jobs.len(), 83);
    let j = &jobs[0];
    assert_eq!(
        j.slug,
        "lever:umbrella:0b6ba409-8f42-449c-8cc8-8ab9bd4a4627"
    );
    assert_eq!(
        j.url,
        "https://jobs.lever.co/umbrella/0b6ba409-8f42-449c-8cc8-8ab9bd4a4627"
    );
    assert_eq!(j.title, "Security Engineer - Identity Platform");
    assert_eq!(j.company, "umbrella");
    assert_eq!(j.location.as_deref(), Some("London"));
    assert_eq!(j.posted_at.as_deref(), Some("2026-10-13T13:30:33Z"));
    assert_eq!(j.employment_type.as_deref(), Some("full_time"));
    assert_eq!(j.work_mode.as_deref(), Some("hybrid"));
}

#[tokio::test]
async fn lists_ashby() {
    let (_, jobs) = listed("watch-ashby").await;
    assert_eq!(jobs.len(), 25);
    let j = &jobs[0];
    assert_eq!(j.slug, "ashby:initech:19fd4218-140d-474a-918e-b42a9ccf83fa");
    assert_eq!(
        j.url,
        "https://jobs.ashbyhq.com/initech/19fd4218-140d-474a-918e-b42a9ccf83fa"
    );
    assert_eq!(j.title, "Senior Software Engineer");
    assert_eq!(j.posted_at.as_deref(), Some("2026-10-07T16:19:20Z"));
    assert_eq!(j.employment_type.as_deref(), Some("full_time"));
    assert_eq!(j.work_mode.as_deref(), Some("hybrid"));
}

#[tokio::test]
async fn lists_workday_page_by_page() {
    let (_, jobs) = listed("watch-workday").await;
    assert_eq!(jobs.len(), 142);
    let j = &jobs[0];
    assert_eq!(j.slug, "workday:bluth/bluth:R34242");
    assert_eq!(j.external_id.as_deref(), Some("bluth/bluth:R34242"));
    assert_eq!(
        j.url,
        "https://bluth.wd1.myworkdayjobs.com/bluth/job/Manila-Philippines/Customer-Support-Sr-Specialist---Billing-Operations_R34242"
    );
    assert_eq!(
        j.title,
        "Customer Support Sr. Specialist - Billing Operations"
    );
    assert_eq!(j.company, "bluth");
    assert_eq!(j.location.as_deref(), Some("Manila, Philippines"));
    assert_eq!(j.posted_at.as_deref(), Some("2026-10-13T00:00:00Z"));
    assert!(
        jobs.iter()
            .any(|j| j.posted_at.as_deref() == Some("2026-10-09T00:00:00Z"))
    );
    // "Posted 30+ Days Ago".
    assert!(jobs.last().unwrap().posted_at.is_none());
}

#[tokio::test]
async fn lists_smartrecruiters_page_by_page() {
    let (_, jobs) = listed("watch-smartrecruiters").await;
    assert_eq!(jobs.len(), 121);
    let j = &jobs[0];
    assert_eq!(j.slug, "smartrecruiters:SterlingCooper:6100000001022686");
    assert_eq!(
        j.url,
        "https://jobs.smartrecruiters.com/SterlingCooper/6100000001022686"
    );
    assert_eq!(j.title, "Enterprise Account Executive");
    assert_eq!(j.company, "Sterling Cooper");
    assert_eq!(j.location.as_deref(), Some("Sydney, Australia"));
    assert_eq!(j.posted_at.as_deref(), Some("2026-10-14T02:24:36Z"));
    assert_eq!(j.employment_type.as_deref(), Some("full_time"));
}

#[tokio::test]
async fn lists_workable() {
    let (_, jobs) = listed("watch-workable").await;
    assert_eq!(jobs.len(), 6);
    let j = &jobs[0];
    assert_eq!(j.slug, "workable:soylent:C41E8D2F90");
    assert_eq!(j.url, "https://apply.workable.com/soylent/j/C41E8D2F90/");
    assert_eq!(
        j.title,
        "Senior Python Engineer, Developer Tooling - EMEA Remote"
    );
    assert_eq!(j.company, "Soylent AI");
    assert_eq!(j.posted_at.as_deref(), Some("2026-09-21T00:00:00Z"));
}

#[tokio::test]
async fn lists_recruitee() {
    let (_, jobs) = listed("watch-recruitee").await;
    assert_eq!(jobs.len(), 21);
    let j = &jobs[0];
    assert_eq!(
        j.slug,
        "recruitee:gringotts:senior-product-manager-vaults-1"
    );
    assert_eq!(
        j.url,
        "https://gringotts.recruitee.com/o/senior-product-manager-vaults-1"
    );
    assert_eq!(j.title, "(Senior) Product Manager, Vaults");
    assert_eq!(j.company, "Gringotts");
    assert_eq!(
        j.location.as_deref(),
        Some("Rotterdam, Zuid-Holland, Netherlands")
    );
    assert_eq!(j.posted_at.as_deref(), Some("2026-10-13T10:58:56Z"));
    assert_eq!(j.work_mode.as_deref(), Some("hybrid"));
}

/// Answers every request with one status.
struct Status(u16);

impl Fetcher for Status {
    async fn get(&self, url: &str) -> Result<Response, String> {
        Ok(Response {
            url: url.to_owned(),
            status: self.0,
            content_type: None,
            retry_after: None,
            body: String::new(),
        })
    }

    async fn post_json(
        &self,
        url: &str,
        _body: &str,
        _timeout: Duration,
    ) -> Result<Response, String> {
        self.get(url).await
    }
}

#[tokio::test]
async fn maps_failures() {
    let board = Board::parse("boards.greenhouse.io/nosuchboard").unwrap();
    let err = list(&Status(404), &board, NOW).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "https://job-boards.greenhouse.io/nosuchboard isn't a board gliide can read: \
         boards-api.greenhouse.io has no greenhouse board by that name (HTTP 404)"
    );
    let workday = Board::parse("bluth.wd1.myworkdayjobs.com/nosuch").unwrap();
    let err = list(&Status(404), &workday, NOW).await.unwrap_err();
    assert!(matches!(err, IngestError::NotABoard { .. }), "{err:?}");
    let err = list(&Status(429), &board, NOW).await.unwrap_err();
    assert!(
        matches!(&err, IngestError::RateLimited { host, .. } if host == "boards-api.greenhouse.io"),
        "{err:?}"
    );
    let err = list(&Status(503), &workday, NOW).await.unwrap_err();
    assert!(
        matches!(&err, IngestError::Network { host, .. } if host == "bluth.wd1.myworkdayjobs.com"),
        "{err:?}"
    );
}
