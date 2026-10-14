//! Reading a feed job's posting, whichever origin found it (ARCHITECTURE.md §7, Job feed).

use resume_core::feed::{FeedJob, Origin};
use resume_core::{Method, Posting};

use crate::{Fetcher, IngestError, freehire, read_with, text};

/// The job's posting. freehire's jobs: [`freehire::read_job`]. LinkedIn and watchlist jobs: their link, read as
/// pasting it would (same text, same hash). Fantastic.jobs' jobs: their employer's link, else the text the index
/// lists when that page can't be read (not when the network or a rate limit failed). `source` = the job's page at its
/// origin, `url` = the employer's link; title, company, place and pay fall back to the listing's.
pub async fn read_job(fetcher: &impl Fetcher, job: &FeedJob) -> Result<Posting, IngestError> {
    let mut posting = match job.origin {
        Origin::Freehire => return freehire::read_job(fetcher, job).await,
        Origin::Linkedin | Origin::Watchlist => read_with(fetcher, &job.url).await?,
        Origin::Fantastic => match read_with(fetcher, &job.url).await {
            Ok(p) => p,
            Err(e @ (IngestError::Network { .. } | IngestError::RateLimited { .. })) => {
                return Err(e);
            }
            Err(e) => {
                let listed = job.description.as_deref().map(text::tidy_pasted);
                match listed.filter(|t| !t.is_empty()) {
                    Some(text) => {
                        Posting::new(Method::Fantastic, None, None, None, None, None, text)
                    }
                    None => return Err(e),
                }
            }
        },
    };
    posting.source = Some(job.page_url());
    posting.url = Some(job.url.clone());
    posting.title = posting.title.or_else(|| Some(job.title.clone()));
    posting.company = posting
        .company
        .or_else(|| Some(job.company.clone()).filter(|c| freehire::is_name(c)));
    posting.location = posting.location.or_else(|| job.location.clone());
    posting.salary = posting.salary.or_else(|| job.salary.clone());
    Ok(posting)
}
