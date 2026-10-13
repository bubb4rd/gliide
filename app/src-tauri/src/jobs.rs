//! Jobs: postings read from a link or pasted text, their requirements (Extract, cached by `extract_key` + model) and
//! every master scored against them.

use std::sync::Arc;

use resume_core::requirements::Seniority;
use resume_core::{Method, Posting, Requirements, rank, score};
use resume_llm::extract;
use resume_pipeline::{extract_key, today};
use resume_store::{PostingRow, Store};

use crate::dto::{ChipState, JobHead, JobRow, JobView, MasterChoice, SkillChip, Stage};
use crate::error::{AppError, Result};
use crate::state::{App, Library, blocking, lock, model_call};

/// Every posting, newest first.
pub fn jobs(app: &App) -> Result<Vec<JobRow>> {
    let rows = lock(&app.store).postings()?;
    let now = app.masters_now()?;
    rows.iter().map(|r| job_row(app, r, now.as_ref())).collect()
}

/// Reads `input` (a link or the posting's text) into a posting; the same text read again is the same posting.
pub async fn add_job(app: &Arc<App>, input: &str) -> Result<JobRow> {
    let posting = resume_ingest::read(input).await?;
    blocking(app, move |app| {
        let row = {
            let store = lock(&app.store);
            let id = store.put_posting(&posting)?;
            store.posting(&id)?.ok_or_else(AppError::no_posting)?
        };
        job_row(app, &row, app.masters_now()?.as_ref())
    })
    .await
}

/// The posting with its requirements and every master scored, best first.
pub async fn open_job(app: &Arc<App>, job: &str) -> Result<JobView> {
    let row = posting(app, job)?;
    let req = requirements(app, &row).await?;
    blocking(app, move |app| {
        let now = app.masters_now()?;
        let masters = ranked(now.as_ref(), &req);
        let best = now.as_ref().and_then(|n| {
            let id = &masters.first()?.id;
            n.masters.iter().find(|(_, m, _)| &m.id == id)
        });
        let skills = match best {
            Some((_, _, page)) => score(&page.resume, &req, &page.facts, today())
                .skills
                .iter()
                .map(|s| SkillChip {
                    term: s.term.clone(),
                    priority: s.priority,
                    required: s.required,
                    state: if s.status.literal() {
                        ChipState::Have
                    } else {
                        ChipState::Missing
                    },
                    locked: None,
                })
                .collect(),
            None => req
                .skills
                .iter()
                .map(|s| SkillChip {
                    term: s.term.clone(),
                    priority: s.priority,
                    required: s.required,
                    state: ChipState::Missing,
                    locked: None,
                })
                .collect(),
        };
        Ok(JobView {
            job: job_row(app, &row, now.as_ref())?,
            head: head(&row.posting, Some(&req)),
            masters,
            skills,
            knockouts: req.knockouts.iter().map(|k| k.text.clone()).collect(),
        })
    })
    .await
}

/// Deletes the posting (its reviews too; its application stays, without the link to it). Every posting left.
pub fn delete_job(app: &App, job: &str) -> Result<Vec<JobRow>> {
    app.cancel(job);
    {
        let mut review = lock(&app.review);
        if review.as_ref().is_some_and(|r| r.posting_id == job) {
            *review = None;
        }
    }
    lock(&app.store).delete_posting(job)?;
    jobs(app)
}

pub fn posting(app: &App, job: &str) -> Result<PostingRow> {
    lock(&app.store)
        .posting(job)?
        .ok_or_else(AppError::no_posting)
}

/// The posting's requirements: from the store when this text, prompt and model were extracted before, else one
/// Extract call (cancellable under the posting's id).
pub async fn requirements(app: &Arc<App>, row: &PostingRow) -> Result<Requirements> {
    let model = app.settings().extract_model;
    let key = extract_key(&row.posting);
    if let Some(req) = lock(&app.store).requirements(&key, &model)? {
        return Ok(req);
    }
    let llm = app.claude().await?;
    let posting = row.posting.clone();
    let m = model.clone();
    let (req, out) = model_call(app, &row.id, Stage::Extracting, async move {
        extract::extract(&llm, &extract::schema(), &posting, &m).await
    })
    .await?;
    lock(&app.store).put_requirements(
        &row.id,
        &key,
        &model,
        &req,
        out.latency().as_millis() as u64,
    )?;
    Ok(req)
}

/// Every master's score now against `req`, best first.
pub fn ranked(now: Option<&Library>, req: &Requirements) -> Vec<MasterChoice> {
    let Some(now) = now else {
        return vec![];
    };
    let candidates: Vec<_> = now
        .masters
        .iter()
        .map(|(_, m, p)| (m, p.resume.clone(), p.facts.clone()))
        .collect();
    rank(&candidates, req, today())
        .into_iter()
        .map(|(m, score)| MasterChoice {
            id: m.id.clone(),
            title: m.title.clone(),
            score,
        })
        .collect()
}

/// The posting as the Jobs list shows it: the requirements' names once extracted, its latest review's projected
/// score (else the best master's score now), and its application's status.
pub fn job_row(app: &App, row: &PostingRow, now: Option<&Library>) -> Result<JobRow> {
    let (req, projected, status) = {
        let store = lock(&app.store);
        (
            store.latest_requirements(&row.id)?,
            store.latest_analysis(&row.id)?.and_then(|a| a.projected),
            store.application_for_posting(&row.id)?.map(|a| a.status),
        )
    };
    let score = projected.or_else(|| {
        let req = req.as_ref()?;
        ranked(now, req).first().map(|m| m.score)
    });
    Ok(row_of(row, req.as_ref(), score, status.map(Into::into)))
}

/// [`job_row`] from what the caller already has.
pub fn row_of(
    row: &PostingRow,
    req: Option<&Requirements>,
    score: Option<f64>,
    status: Option<crate::dto::AppStatus>,
) -> JobRow {
    let h = head(&row.posting, req);
    JobRow {
        id: row.id.clone(),
        role: h.role,
        company: h.company,
        location: h.location,
        url: h.url,
        score,
        status,
        added_at: row.created_at.clone(),
    }
}

/// The application status for a posting, as the UI names it.
pub fn status_of(store: &Store, posting_id: &str) -> Result<Option<crate::dto::AppStatus>> {
    Ok(store
        .application_for_posting(posting_id)?
        .map(|a| a.status.into()))
}

/// The job header: Extract's names when there are requirements, else the source's metadata.
pub fn head(p: &Posting, req: Option<&Requirements>) -> JobHead {
    let pick = |from_req: Option<&str>, from_posting: &Option<String>| {
        from_req
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                from_posting
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or_default()
            .to_owned()
    };
    let mut role = pick(req.map(|r| r.role.as_str()), &p.title);
    if role.is_empty() {
        role = "Untitled posting".to_owned();
    }
    JobHead {
        role,
        company: pick(req.map(|r| r.company.as_str()), &p.company),
        location: pick(req.map(|r| r.location.as_str()), &p.location),
        seniority: req.map_or("", |r| seniority(r.seniority)).to_owned(),
        url: p.url.clone().or_else(|| p.source.clone()),
        source: source(p),
    }
}

fn seniority(s: Seniority) -> &'static str {
    match s {
        Seniority::Intern => "Intern",
        Seniority::Entry => "Entry level",
        Seniority::Mid => "Mid level",
        Seniority::Senior => "Senior",
        Seniority::Lead => "Lead",
    }
}

/// `pasted`, `from Greenhouse`, `read from globex.com`.
fn source(p: &Posting) -> String {
    let board = match p.method {
        Method::Pasted => return "pasted".to_owned(),
        Method::Greenhouse => "Greenhouse",
        Method::Lever => "Lever",
        Method::Ashby => "Ashby",
        Method::Linkedin => "LinkedIn",
        Method::JsonLd | Method::Html => {
            let host = p
                .url
                .as_deref()
                .or(p.source.as_deref())
                .and_then(host)
                .unwrap_or_default();
            return if host.is_empty() {
                "read from the web".to_owned()
            } else {
                format!("read from {host}")
            };
        }
    };
    format!("from {board}")
}

/// `https://www.globex.com/careers/…` → `globex.com`.
fn host(url: &str) -> Option<&str> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = host.split(':').next()?;
    Some(host.strip_prefix("www.").unwrap_or(host)).filter(|h| !h.is_empty())
}

impl From<resume_store::AppStatus> for crate::dto::AppStatus {
    fn from(s: resume_store::AppStatus) -> Self {
        use resume_store::AppStatus as S;
        match s {
            S::Saved => Self::Saved,
            S::Applied => Self::Applied,
            S::Interviewing => Self::Interviewing,
            S::Offer => Self::Offer,
            S::Rejected => Self::Rejected,
        }
    }
}

impl From<crate::dto::AppStatus> for resume_store::AppStatus {
    fn from(s: crate::dto::AppStatus) -> Self {
        use crate::dto::AppStatus as S;
        match s {
            S::Saved => Self::Saved,
            S::Applied => Self::Applied,
            S::Interviewing => Self::Interviewing,
            S::Offer => Self::Offer,
            S::Rejected => Self::Rejected,
        }
    }
}
