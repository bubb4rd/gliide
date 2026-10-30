//! The open review's cover letter: written by the model from the tailored resume as the
//! draft has it now (your toggles and edits included) and the posting; each part yours to edit; one paragraph
//! rewritten with a note; rendered under the resume's letterhead by one warm renderer. Stored per analysis.

use std::sync::Arc;

use resume_core::Resume;
use resume_llm::cover::{self, CoverInput};
use resume_render::cover::{LetterContact, LetterData};
use resume_render::{FontOptions, Renderer};
use resume_store::Cover;

use crate::dto::{CoverPart, CoverView, Letter, Stage};
use crate::error::{AppError, Result};
use crate::review::Review;
use crate::state::{App, blocking, lock, model_call};

/// The letter's renderer's key in [`App::renderers`] (beside the resume templates').
const TEMPLATE: &str = "cover-letter";

/// What a model call reads, copied out of the open review so no lock is held across it.
struct Job {
    posting_id: String,
    analysis_id: String,
    company: String,
    role: String,
    resume: Resume,
    posting: String,
    letter: Option<Cover>,
}

impl Job {
    fn input(&self) -> CoverInput<'_> {
        CoverInput {
            company: &self.company,
            role: &self.role,
            resume: &self.resume,
            posting: &self.posting,
        }
    }
}

/// The open review `id`.
fn open<'a>(slot: &'a Option<Review>, id: &str) -> Result<&'a Review> {
    slot.as_ref()
        .filter(|r| r.id == id)
        .ok_or_else(AppError::replaced)
}

/// The company the letter is addressed to: Extract's, else the posting's.
pub fn company(r: &Review) -> &str {
    non_empty(&r.req.company)
        .or(r.posting.posting.company.as_deref())
        .unwrap_or("")
}

/// The role by its title: Extract's, else the posting's.
pub fn role(r: &Review) -> &str {
    non_empty(&r.req.role)
        .or(r.posting.posting.title.as_deref())
        .unwrap_or("")
}

fn non_empty(s: &str) -> Option<&str> {
    Some(s.trim()).filter(|s| !s.is_empty())
}

/// The tailored resume as the last rebuild made it.
fn tailored(r: &Review) -> Result<&Resume> {
    Ok(&r.last.as_ref().ok_or_else(AppError::replaced)?.t.resume)
}

fn job(app: &App, review: &str) -> Result<Job> {
    let slot = lock(&app.review);
    let r = open(&slot, review)?;
    let letter = lock(&app.store).cover(&r.id)?;
    Ok(Job {
        posting_id: r.posting_id.clone(),
        analysis_id: r.id.clone(),
        company: company(r).to_owned(),
        role: role(r).to_owned(),
        resume: tailored(r)?.clone(),
        posting: r.posting.posting.text.clone(),
        letter,
    })
}

/// `letter` compiled under the review's letterhead, dated today, through the warm letter renderer.
pub fn render<T>(
    app: &App,
    r: &Review,
    letter: &Cover,
    f: impl FnOnce(&resume_render::Rendered) -> Result<T>,
) -> Result<T> {
    let resume = tailored(r)?;
    let date = chrono::Local::now().format("%B %-d, %Y").to_string();
    let data = LetterData {
        name: &resume.name,
        contact: resume
            .contact
            .iter()
            .map(|c| LetterContact {
                text: &c.text,
                url: &c.url,
            })
            .collect(),
        date: &date,
        company: company(r),
        role: role(r),
        greeting: &letter.greeting,
        paragraphs: &letter.paragraphs,
        closing: &letter.closing,
    };
    let json = serde_json::to_string(&data)?;
    let mut pool = lock(&app.renderers);
    let renderer = pool.entry(TEMPLATE.to_owned()).or_insert_with(|| {
        Renderer::new(&resume_render::cover::template(), FontOptions::default())
    });
    f(&renderer.render(&json, false)?)
}

/// The letter of the open review `review` and its pages; no letter yet gives an empty view.
pub fn cover_view(app: &App, review: &str) -> Result<CoverView> {
    let slot = lock(&app.review);
    let r = open(&slot, review)?;
    let letter = lock(&app.store).cover(&r.id)?;
    view(app, r, letter)
}

fn view(app: &App, r: &Review, letter: Option<Cover>) -> Result<CoverView> {
    let Some(letter) = letter else {
        return Ok(CoverView {
            review: r.id.clone(),
            letter: None,
            pages: Vec::new(),
            page_count: 0,
        });
    };
    let (pages, page_count) = render(app, r, &letter, |d| Ok((d.svgs(), d.metrics.pages)))?;
    let words = letter
        .paragraphs
        .iter()
        .map(|p| p.split_whitespace().count())
        .sum::<usize>();
    Ok(CoverView {
        review: r.id.clone(),
        letter: Some(Letter {
            greeting: letter.greeting,
            paragraphs: letter.paragraphs,
            closing: letter.closing,
            words: u32::try_from(words).unwrap_or(u32::MAX),
        }),
        pages,
        page_count,
    })
}

/// Saves `letter` for the analysis it was written for, then shows it when that review is still the open one.
fn save_and_view(
    app: &App,
    review: &str,
    greeting: &str,
    paragraphs: &[String],
    closing: &str,
) -> Result<CoverView> {
    let slot = lock(&app.review);
    let saved = lock(&app.store).put_cover(review, greeting, paragraphs, closing)?;
    let r = open(&slot, review)?;
    view(app, r, Some(saved))
}

/// One model call: the whole letter, written again when there is one.
pub async fn write_cover(app: &Arc<App>, review: &str) -> Result<CoverView> {
    let id = review.to_owned();
    let job = blocking(app, move |app| job(app, &id)).await?;
    let llm = app.agent().await?;
    let model = app.settings().current().suggest_model.clone();
    let posting_id = job.posting_id.clone();
    let m = model.clone();
    let (letter, out, job) = model_call(app, &posting_id, Stage::Writing, async move {
        let (letter, out) = cover::write(&llm, &cover::schema(), &job.input(), &m).await?;
        Ok((letter, out, job))
    })
    .await?;
    app.note_served(&model, &out);
    blocking(app, move |app| {
        save_and_view(
            app,
            &job.analysis_id,
            &letter.greeting,
            &letter.paragraphs,
            &letter.closing,
        )
    })
    .await
}

/// Sets one part of the letter to your own words.
pub fn set_cover_part(app: &App, review: &str, part: CoverPart, text: &str) -> Result<CoverView> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::other("Write the text first"));
    }
    let slot = lock(&app.review);
    let r = open(&slot, review)?;
    let store = lock(&app.store);
    let mut letter = store
        .cover(&r.id)?
        .ok_or_else(|| AppError::missing("Write the cover letter first"))?;
    match part {
        CoverPart::Greeting => letter.greeting = text.to_owned(),
        CoverPart::Closing => letter.closing = text.to_owned(),
        CoverPart::Paragraph { index } => {
            *paragraph(&mut letter, index)? = text.to_owned();
        }
    }
    let saved = store.put_cover(&r.id, &letter.greeting, &letter.paragraphs, &letter.closing)?;
    drop(store);
    view(app, r, Some(saved))
}

fn paragraph(letter: &mut Cover, index: u32) -> Result<&mut String> {
    letter
        .paragraphs
        .get_mut(index as usize)
        .ok_or_else(|| AppError::missing("This paragraph is gone; reopen the letter"))
}

/// One model call: paragraph `index` rewritten per `note` (may be empty), in step with the rest of the letter.
pub async fn rewrite_cover_paragraph(
    app: &Arc<App>,
    review: &str,
    index: u32,
    note: String,
) -> Result<CoverView> {
    let id = review.to_owned();
    let job = blocking(app, move |app| job(app, &id)).await?;
    let Some(before) = job.letter.clone() else {
        return Err(AppError::missing("Write the cover letter first"));
    };
    if index as usize >= before.paragraphs.len() {
        return Err(AppError::missing(
            "This paragraph is gone; reopen the letter",
        ));
    }
    let llm = app.agent().await?;
    let model = app.settings().current().suggest_model.clone();
    let posting_id = job.posting_id.clone();
    let m = model.clone();
    let (text, out, job) = model_call(app, &posting_id, Stage::Revising, async move {
        let letter = cover::Letter {
            greeting: before.greeting,
            paragraphs: before.paragraphs,
            closing: before.closing,
        };
        let (text, out) = cover::rewrite_paragraph(
            &llm,
            &cover::paragraph_schema(),
            &job.input(),
            &letter,
            index as usize,
            &note,
            &m,
        )
        .await?;
        Ok((text, out, job))
    })
    .await?;
    app.note_served(&model, &out);
    blocking(app, move |app| {
        let mut letter = lock(&app.store)
            .cover(&job.analysis_id)?
            .ok_or_else(|| AppError::missing("Write the cover letter first"))?;
        *paragraph(&mut letter, index)? = text;
        save_and_view(
            app,
            &job.analysis_id,
            &letter.greeting,
            &letter.paragraphs,
            &letter.closing,
        )
    })
    .await
}
