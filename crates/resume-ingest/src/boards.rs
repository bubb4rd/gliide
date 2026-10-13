//! Job board APIs: Greenhouse, Lever, Ashby, and LinkedIn's guest job page.

use scraper::{Html, Selector};
use serde::Deserialize;

use crate::Found;
use crate::text::{html_to_text, unescape};
use resume_core::Method;

/// `boards-api.greenhouse.io/v1/boards/{board}/jobs/{id}`; `content` is HTML, escaped once more.
pub(crate) fn greenhouse(body: &str) -> Option<Found> {
    #[derive(Deserialize)]
    struct Job {
        title: String,
        company_name: Option<String>,
        location: Option<Named>,
        absolute_url: Option<String>,
        content: String,
    }
    #[derive(Deserialize)]
    struct Named {
        name: Option<String>,
    }
    let job: Job = serde_json::from_str(body).ok()?;
    Some(Found {
        method: Method::Greenhouse,
        url: job.absolute_url,
        title: clean(&job.title),
        company: job.company_name.as_deref().and_then(clean),
        location: job.location.and_then(|l| l.name).as_deref().and_then(clean),
        text: html_to_text(&unescape(&job.content)),
    })
}

/// `api.lever.co/v0/postings/{company}/{id}`: an opening, titled lists of `<li>` items, and a closing section. The
/// API doesn't name the company.
pub(crate) fn lever(body: &str) -> Option<Found> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Posting {
        text: String,
        #[serde(default)]
        categories: Categories,
        workplace_type: Option<String>,
        hosted_url: Option<String>,
        #[serde(default)]
        description: String,
        #[serde(default)]
        lists: Vec<List>,
        #[serde(default)]
        additional: String,
    }
    #[derive(Deserialize, Default)]
    #[serde(rename_all = "camelCase")]
    struct Categories {
        location: Option<String>,
        #[serde(default)]
        all_locations: Vec<String>,
    }
    #[derive(Deserialize)]
    struct List {
        text: String,
        content: String,
    }
    let p: Posting = serde_json::from_str(body).ok()?;
    let mut sections = vec![html_to_text(&p.description)];
    for list in &p.lists {
        let items = html_to_text(&list.content);
        sections.push(match clean(&list.text) {
            Some(heading) => format!("{heading}\n{items}"),
            None => items,
        });
    }
    sections.push(html_to_text(&p.additional));
    let places = if p.categories.all_locations.is_empty() {
        p.categories.location.into_iter().collect()
    } else {
        p.categories.all_locations
    };
    Some(Found {
        method: Method::Lever,
        url: p.hosted_url,
        title: clean(&p.text),
        company: None,
        location: place(&places, p.workplace_type.as_deref()),
        text: join_sections(sections),
    })
}

/// `api.ashbyhq.com/posting-api/job-board/{org}`: every listed job of the board; the one with `id` is the posting.
/// The API doesn't name the company. `Err` says why there is no posting.
pub(crate) fn ashby(body: &str, id: &str) -> Result<Found, String> {
    #[derive(Deserialize)]
    struct Board {
        jobs: Vec<Job>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Job {
        id: String,
        title: String,
        location: Option<String>,
        #[serde(default)]
        secondary_locations: Vec<Secondary>,
        workplace_type: Option<String>,
        job_url: Option<String>,
        #[serde(default)]
        description_html: String,
    }
    #[derive(Deserialize)]
    struct Secondary {
        location: Option<String>,
    }
    let board: Board =
        serde_json::from_str(body).map_err(|_| "Ashby returned no job board".to_owned())?;
    let job = board
        .jobs
        .into_iter()
        .find(|j| j.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| {
            "the job isn't on the company's Ashby board; it may have closed".to_owned()
        })?;
    let places: Vec<String> = job
        .location
        .into_iter()
        .chain(
            job.secondary_locations
                .into_iter()
                .filter_map(|s| s.location),
        )
        .collect();
    Ok(Found {
        method: Method::Ashby,
        url: job.job_url,
        title: clean(&job.title),
        company: None,
        location: place(&places, job.workplace_type.as_deref()),
        text: html_to_text(&job.description_html),
    })
}

/// LinkedIn's guest job page (`/jobs-guest/jobs/api/jobPosting/{id}`): a top card (title, company, place), the
/// description markup, and criteria (seniority, employment type…), which are kept as `Label: value` lines.
pub(crate) fn linkedin(body: &str) -> Option<Found> {
    let doc = Html::parse_document(body);
    let first = |css: &str| {
        let sel = Selector::parse(css).expect("static selector");
        doc.select(&sel)
            .next()
            .and_then(|el| clean(&el.text().collect::<String>()))
    };
    let description = {
        let sel = Selector::parse(".show-more-less-html__markup").expect("static selector");
        doc.select(&sel).next()?.inner_html()
    };
    let mut text = html_to_text(&description);
    let item = Selector::parse(".description__job-criteria-item").expect("static selector");
    let label = Selector::parse(".description__job-criteria-subheader").expect("static selector");
    let value = Selector::parse(".description__job-criteria-text").expect("static selector");
    let criteria: Vec<String> = doc
        .select(&item)
        .filter_map(|it| {
            let l = clean(&it.select(&label).next()?.text().collect::<String>())?;
            let v = clean(&it.select(&value).next()?.text().collect::<String>())?;
            Some(format!("{l}: {v}"))
        })
        .collect();
    if !criteria.is_empty() {
        text = join_sections(vec![text, criteria.join("\n")]);
    }
    Some(Found {
        method: Method::Linkedin,
        url: None,
        title: first(".topcard__title"),
        company: first(".topcard__org-name-link").or_else(|| first(".topcard__flavor")),
        location: first(".topcard__flavor-row .topcard__flavor--bullet"),
        text,
    })
}

/// Places joined with `; `, with the workplace kind when it adds something: `Dallas; Seattle (hybrid)`,
/// `Remote`.
pub(crate) fn place(places: &[String], workplace: Option<&str>) -> Option<String> {
    let mut seen: Vec<String> = Vec::new();
    for p in places.iter().filter_map(|p| clean(p)) {
        if !seen.contains(&p) {
            seen.push(p);
        }
    }
    let joined = seen.join("; ");
    let kind = match workplace.map(str::to_ascii_lowercase).as_deref() {
        Some("remote") => "remote",
        Some("hybrid") => "hybrid",
        Some("onsite" | "on-site" | "on_site") => "onsite",
        _ => "",
    };
    match (joined.is_empty(), kind) {
        (true, "") => None,
        (true, "remote") => Some("Remote".to_owned()),
        (true, _) => None,
        (false, "") => Some(joined),
        (false, k) if joined.to_ascii_lowercase().contains(k) => Some(joined),
        (false, k) => Some(format!("{joined} ({k})")),
    }
}

/// Collapsed, entity-decoded single-line metadata; `None` when empty.
pub(crate) fn clean(s: &str) -> Option<String> {
    let s = unescape(s);
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    (!s.is_empty()).then_some(s)
}

/// Non-empty sections, a blank line between them.
pub(crate) fn join_sections(sections: Vec<String>) -> String {
    sections
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
