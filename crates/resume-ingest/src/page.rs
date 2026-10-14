//! Any other page: its schema.org `JobPosting` JSON-LD (Workday, iCIMS, ApplicantPro…), else its main text.

use std::sync::LazyLock;

use scraper::{Html, Selector};
use serde_json::Value;

use crate::Found;
use crate::boards::{clean, place};
use crate::text::{html_to_text, page_text, unescape};
use resume_core::Method;
use resume_core::salary::{self, Salary};

/// The posting in a page: from JSON-LD when that has at least `min_chars` of description, else the page's main
/// text. Either way `url` is the page's canonical link when it names one.
pub(crate) fn read(html: &str, min_chars: usize) -> Found {
    let doc = Html::parse_document(html);
    let canonical = canonical(&doc);
    if let Some(job) = json_ld(&doc)
        && job.text.chars().count() >= min_chars
    {
        return Found {
            url: canonical.or(job.url),
            ..job
        };
    }
    Found {
        method: Method::Html,
        url: canonical.or_else(|| meta(&doc, "og:url")),
        title: meta(&doc, "og:title").or_else(|| first_text(&doc, "title")),
        company: meta(&doc, "og:site_name"),
        location: None,
        salary: None,
        text: main_text(&doc),
    }
}

/// The first JSON-LD `JobPosting` with a description: the node itself, an array item or an `@graph` member.
fn json_ld(doc: &Html) -> Option<Found> {
    json_ld_values(doc).find_map(|v| find_job(&v, 0).and_then(job_posting))
}

/// The pay in a page's JSON-LD `JobPosting` (`baseSalary`): for boards read from their own data whose pages also
/// carry it (Ashby).
pub(crate) fn salary(html: &str) -> Option<Salary> {
    let doc = Html::parse_document(html);
    json_ld_values(&doc).find_map(|v| find_job(&v, 0)?.get("baseSalary").and_then(base_salary))
}

/// Every JSON-LD block in `doc` that parses.
fn json_ld_values(doc: &Html) -> impl Iterator<Item = Value> + '_ {
    static SCRIPTS: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("script").expect("static selector"));
    doc.select(&SCRIPTS)
        .filter(|s| {
            s.value()
                .attr("type")
                .is_some_and(|t| t.to_ascii_lowercase().contains("ld+json"))
        })
        .filter_map(|s| {
            let raw: String = s.text().collect();
            // Raw line breaks inside strings are a common fault; outside strings they're whitespace anyway.
            serde_json::from_str::<Value>(&raw)
                .or_else(|_| serde_json::from_str(&raw.replace(['\n', '\r'], " ")))
                .ok()
        })
}

fn find_job(v: &Value, depth: u8) -> Option<&Value> {
    if depth > 4 {
        return None;
    }
    match v {
        Value::Object(o) => {
            let is_job = match o.get("@type") {
                Some(Value::String(t)) => t == "JobPosting",
                Some(Value::Array(ts)) => ts.iter().any(|t| t == "JobPosting"),
                _ => false,
            };
            if is_job {
                return Some(v);
            }
            o.get("@graph").and_then(|g| find_job(g, depth + 1))
        }
        Value::Array(items) => items.iter().find_map(|i| find_job(i, depth + 1)),
        _ => None,
    }
}

fn job_posting(job: &Value) -> Option<Found> {
    let description = job.get("description")?.as_str()?;
    // Some boards escape the HTML once more inside the JSON.
    let description = if !description.contains('<') && description.contains("&lt;") {
        unescape(description)
    } else {
        description.to_owned()
    };
    let org = job.get("hiringOrganization");
    let company = org
        .and_then(|o| o.get("name").or(Some(o)))
        .and_then(Value::as_str)
        .and_then(clean);
    let places: Vec<String> = match job.get("jobLocation") {
        Some(Value::Array(ls)) => ls.iter().filter_map(address).collect(),
        Some(l) => address(l).into_iter().collect(),
        None => Vec::new(),
    };
    let remote = job
        .get("jobLocationType")
        .and_then(Value::as_str)
        .is_some_and(|t| t.eq_ignore_ascii_case("TELECOMMUTE"));
    Some(Found {
        method: Method::JsonLd,
        url: job.get("url").and_then(Value::as_str).map(str::to_owned),
        title: job.get("title").and_then(Value::as_str).and_then(clean),
        company,
        location: place(&places, remote.then_some("remote")),
        salary: job.get("baseSalary").and_then(base_salary),
        text: html_to_text(&description),
    })
}

/// A `MonetaryAmount`: `{ currency, value: { minValue, maxValue, value, unitText } }`, or `value` a bare number.
/// Numbers may be written as strings.
fn base_salary(amount: &Value) -> Option<Salary> {
    let number = |v: Option<&Value>| match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.replace(',', "").trim().parse().ok(),
        _ => None,
    };
    let currency = amount
        .get("currency")
        .and_then(Value::as_str)
        .unwrap_or("USD");
    let value = amount.get("value");
    let (min, max, unit) = match value {
        Some(q @ Value::Object(_)) => {
            let exact = number(q.get("value"));
            (
                number(q.get("minValue")).or(exact),
                number(q.get("maxValue")).or(exact),
                q.get("unitText")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        }
        v => (number(v), number(v), ""),
    };
    salary::from_source(min, max, currency, unit)
}

/// `Locality, Region`, or the country when neither is given.
fn address(location: &Value) -> Option<String> {
    let a = location.get("address")?;
    let field = |k: &str| {
        a.get(k)
            .and_then(|v| v.as_str().or_else(|| v.get("name").and_then(Value::as_str)))
            .and_then(clean)
    };
    let parts: Vec<String> = [field("addressLocality"), field("addressRegion")]
        .into_iter()
        .flatten()
        .collect();
    if parts.is_empty() {
        field("addressCountry")
    } else {
        Some(parts.join(", "))
    }
}

/// `<main>`, else `[role=main]`, else `<article>`, else `<body>` without its header.
fn main_text(doc: &Html) -> String {
    for (css, keep_header) in [
        ("main", true),
        ("[role=main]", true),
        ("article", true),
        ("body", false),
    ] {
        let sel = Selector::parse(css).expect("static selector");
        if let Some(el) = doc.select(&sel).next() {
            return page_text(el, keep_header);
        }
    }
    html_to_text(&doc.root_element().inner_html())
}

fn canonical(doc: &Html) -> Option<String> {
    let sel = Selector::parse("link[rel=canonical]").expect("static selector");
    doc.select(&sel)
        .find_map(|l| l.value().attr("href"))
        .filter(|h| h.starts_with("http"))
        .map(str::to_owned)
}

fn meta(doc: &Html, property: &str) -> Option<String> {
    let sel = Selector::parse("meta").expect("static selector");
    doc.select(&sel)
        .find(|m| m.value().attr("property").or(m.value().attr("name")) == Some(property))
        .and_then(|m| m.value().attr("content"))
        .and_then(clean)
}

fn first_text(doc: &Html, css: &str) -> Option<String> {
    let sel = Selector::parse(css).expect("static selector");
    doc.select(&sel)
        .next()
        .and_then(|el| clean(&el.text().collect::<String>()))
}
