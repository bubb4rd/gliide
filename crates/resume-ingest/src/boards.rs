//! Job board APIs and pages: Greenhouse, Lever, Ashby, Workday, SmartRecruiters, Workable, Recruitee, and
//! LinkedIn's guest job page.

use scraper::{Html, Selector};
use serde::Deserialize;

use crate::Found;
use crate::text::{html_to_text, unescape};
use resume_core::{Method, salary};

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
        salary: None,
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
        salary_range: Option<SalaryRange>,
    }
    /// Set when the company publishes pay: `{ min, max, currency, interval: "per-year-salary" }`.
    #[derive(Deserialize)]
    struct SalaryRange {
        min: Option<f64>,
        max: Option<f64>,
        currency: Option<String>,
        interval: Option<String>,
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
        salary: p.salary_range.and_then(|s| {
            salary::from_source(
                s.min,
                s.max,
                s.currency.as_deref().unwrap_or("USD"),
                s.interval.as_deref().unwrap_or_default(),
            )
        }),
        text: join_sections(sections),
    })
}

/// Ashby's job page (`jobs.ashbyhq.com/{org}/{id}`): its `window.__appData` names the organization and holds the
/// posting. Ashby's public board API names no company and lists only listed jobs, so the page is read instead.
/// `None` when the page carries no app data in this shape (then it's read like any other page); `Err` says why there
/// is no posting.
pub(crate) fn ashby(body: &str) -> Option<Result<Found, String>> {
    #[derive(Deserialize)]
    struct AppData {
        organization: Option<Org>,
        posting: Option<Job>,
    }
    #[derive(Deserialize)]
    struct Org {
        name: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Job {
        title: String,
        location_name: Option<String>,
        #[serde(default)]
        secondary_location_names: Vec<String>,
        workplace_type: Option<String>,
        #[serde(default)]
        description_html: String,
    }
    const MARK: &str = "window.__appData";
    let rest = &body[body.find(MARK)? + MARK.len()..];
    let rest = rest.trim_start().strip_prefix('=')?;
    // The object is followed by more script; read the first JSON value only.
    let data: AppData = serde_json::Deserializer::from_str(rest)
        .into_iter()
        .next()?
        .ok()?;
    let Some(job) = data.posting else {
        return Some(Err(
            "Ashby has no such job on the company's board; it may have closed".to_owned(),
        ));
    };
    let places: Vec<String> = job
        .location_name
        .into_iter()
        .chain(job.secondary_location_names)
        .collect();
    Some(Ok(Found {
        method: Method::Ashby,
        url: None,
        title: clean(&job.title),
        company: data
            .organization
            .and_then(|o| o.name)
            .as_deref()
            .and_then(clean),
        location: place(&places, job.workplace_type.as_deref()),
        salary: None,
        text: html_to_text(&job.description_html),
    }))
}

/// Workday's job JSON (`/wday/cxs/{tenant}/{site}/job/{path}`). Its hiring organization is the legal entity, often
/// after an internal number (`2100 NVIDIA USA`), which is dropped.
pub(crate) fn workday(body: &str) -> Option<Found> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Detail {
        job_posting_info: Info,
        hiring_organization: Option<Org>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Info {
        title: String,
        #[serde(default)]
        job_description: String,
        location: Option<String>,
        #[serde(default)]
        additional_locations: Vec<String>,
        remote_type: Option<String>,
        external_url: Option<String>,
    }
    #[derive(Deserialize)]
    struct Org {
        name: Option<String>,
    }
    let d: Detail = serde_json::from_str(body).ok()?;
    let job = d.job_posting_info;
    let company = d.hiring_organization.and_then(|o| o.name).and_then(|n| {
        let n = clean(&n)?;
        match n.split_once(' ') {
            Some((code, rest)) if code.bytes().all(|b| b.is_ascii_digit()) => Some(rest.to_owned()),
            _ => Some(n),
        }
    });
    let places: Vec<String> = job
        .location
        .into_iter()
        .chain(job.additional_locations)
        .collect();
    Some(Found {
        method: Method::Workday,
        url: job.external_url,
        title: clean(&job.title),
        company,
        location: place(&places, job.remote_type.as_deref()),
        salary: None,
        text: html_to_text(&job.job_description),
    })
}

/// `api.smartrecruiters.com/v1/companies/{company}/postings/{id}`: the job ad's titled sections in page order, and
/// pay when the company publishes it.
pub(crate) fn smartrecruiters(body: &str) -> Option<Found> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Posting {
        name: String,
        company: Option<Company>,
        location: Option<Location>,
        posting_url: Option<String>,
        job_ad: Option<JobAd>,
        compensation: Option<Compensation>,
    }
    #[derive(Deserialize)]
    struct Company {
        name: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Location {
        full_location: Option<String>,
        #[serde(default)]
        remote: bool,
        #[serde(default)]
        hybrid: bool,
    }
    #[derive(Deserialize)]
    struct JobAd {
        sections: Sections,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Sections {
        company_description: Option<Section>,
        job_description: Option<Section>,
        qualifications: Option<Section>,
        additional_information: Option<Section>,
    }
    #[derive(Deserialize)]
    struct Section {
        title: Option<String>,
        #[serde(default)]
        text: String,
    }
    /// `{ min, max, currency, period: "YEARLY" }`.
    #[derive(Deserialize)]
    struct Compensation {
        min: Option<f64>,
        max: Option<f64>,
        currency: Option<String>,
        period: Option<String>,
    }
    let p: Posting = serde_json::from_str(body).ok()?;
    let sections = p
        .job_ad
        .map(|ad| {
            let s = ad.sections;
            [
                s.company_description,
                s.job_description,
                s.qualifications,
                s.additional_information,
            ]
            .into_iter()
            .flatten()
            .map(|s| titled(s.title.as_deref(), &s.text))
            .collect()
        })
        .unwrap_or_default();
    let location = p.location.and_then(|l| {
        // `Remote, UNITED STATES, United States`, `Petaling Jaya, , Malaysia`.
        let parts: Vec<&str> = l
            .full_location
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .collect();
        let kept: Vec<&str> = parts
            .iter()
            .enumerate()
            .filter(|(i, s)| {
                !s.is_empty() && !parts[i + 1..].iter().any(|t| t.eq_ignore_ascii_case(s))
            })
            .map(|(_, s)| *s)
            .collect();
        let places: Vec<String> = (!kept.is_empty())
            .then(|| kept.join(", "))
            .into_iter()
            .collect();
        let kind = if l.hybrid {
            Some("hybrid")
        } else if l.remote {
            Some("remote")
        } else {
            None
        };
        place(&places, kind)
    });
    Some(Found {
        method: Method::SmartRecruiters,
        url: p.posting_url,
        title: clean(&p.name),
        company: p.company.and_then(|c| c.name).as_deref().and_then(clean),
        location,
        salary: p.compensation.and_then(|c| {
            salary::from_source(
                c.min,
                c.max,
                c.currency.as_deref()?,
                c.period.as_deref().unwrap_or_default(),
            )
        }),
        text: join_sections(sections),
    })
}

/// `apply.workable.com/api/v2/accounts/{account}/jobs/{shortcode}`: description, requirements and benefits. The API
/// doesn't name the company.
pub(crate) fn workable(body: &str) -> Option<Found> {
    #[derive(Deserialize)]
    struct Job {
        title: String,
        location: Option<Place>,
        #[serde(default)]
        locations: Vec<Place>,
        workplace: Option<String>,
        #[serde(default)]
        description: String,
        #[serde(default)]
        requirements: String,
        #[serde(default)]
        benefits: String,
    }
    #[derive(Deserialize)]
    struct Place {
        city: Option<String>,
        region: Option<String>,
        country: Option<String>,
        #[serde(default)]
        hidden: bool,
    }
    let job: Job = serde_json::from_str(body).ok()?;
    let shown: Vec<Place> = job.locations.into_iter().filter(|p| !p.hidden).collect();
    let places: Vec<String> = if shown.is_empty() {
        job.location.into_iter().collect()
    } else {
        shown
    }
    .into_iter()
    .filter_map(|p| {
        let mut parts: Vec<String> = Vec::new();
        for part in [p.city, p.region, p.country]
            .into_iter()
            .flatten()
            .filter_map(|s| clean(&s))
        {
            if !parts.contains(&part) {
                parts.push(part);
            }
        }
        (!parts.is_empty()).then(|| parts.join(", "))
    })
    .collect();
    Some(Found {
        method: Method::Workable,
        url: None,
        title: clean(&job.title),
        company: None,
        location: place(&places, job.workplace.as_deref()),
        salary: None,
        text: join_sections(vec![
            html_to_text(&job.description),
            titled(Some("Requirements"), &job.requirements),
            titled(Some("Benefits"), &job.benefits),
        ]),
    })
}

/// `{company}.recruitee.com/api/offers/{slug}`: description and requirements, the company's name, and pay when set
/// (its amounts are strings).
pub(crate) fn recruitee(body: &str) -> Option<Found> {
    #[derive(Deserialize)]
    struct One {
        offer: Offer,
    }
    #[derive(Deserialize)]
    struct Offer {
        title: String,
        company_name: Option<String>,
        location: Option<String>,
        #[serde(default)]
        remote: bool,
        #[serde(default)]
        hybrid: bool,
        #[serde(default)]
        on_site: bool,
        careers_url: Option<String>,
        #[serde(default)]
        description: String,
        #[serde(default)]
        requirements: String,
        salary: Option<Pay>,
    }
    #[derive(Deserialize)]
    struct Pay {
        min: Option<serde_json::Value>,
        max: Option<serde_json::Value>,
        period: Option<String>,
        currency: Option<String>,
    }
    fn amount(v: Option<&serde_json::Value>) -> Option<f64> {
        match v? {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
    let o = serde_json::from_str::<One>(body).ok()?.offer;
    let kind = if o.hybrid {
        Some("hybrid")
    } else if o.remote {
        Some("remote")
    } else if o.on_site {
        Some("onsite")
    } else {
        None
    };
    Some(Found {
        method: Method::Recruitee,
        url: o.careers_url,
        title: clean(&o.title),
        company: o.company_name.as_deref().and_then(clean),
        location: place(&o.location.into_iter().collect::<Vec<_>>(), kind),
        salary: o.salary.and_then(|p| {
            salary::from_source(
                amount(p.min.as_ref()),
                amount(p.max.as_ref()),
                p.currency.as_deref()?,
                p.period.as_deref().unwrap_or_default(),
            )
        }),
        text: join_sections(vec![
            html_to_text(&o.description),
            titled(Some("Requirements"), &o.requirements),
        ]),
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
        salary: None,
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

/// An HTML section as text under its heading, unless the text already opens with it; empty when the section is.
fn titled(heading: Option<&str>, html: &str) -> String {
    let text = html_to_text(html);
    match heading.and_then(clean) {
        Some(h)
            if !text.is_empty()
                && !text
                    .lines()
                    .next()
                    .is_some_and(|l| l.eq_ignore_ascii_case(&h)) =>
        {
            format!("{h}\n{text}")
        }
        _ => text,
    }
}
