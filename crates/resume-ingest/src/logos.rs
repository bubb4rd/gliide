//! Logos for feed jobs (ARCHITECTURE.md §7, Company logos): freehire's logo for the company's name, else the
//! company's own site, found through freehire's company page: its `apple-touch-icon`, then `icon`, then
//! `/favicon.ico`. Never an icon from a job board's host: that's the board's.

use std::cmp::Reverse;
use std::sync::LazyLock;

use scraper::{Html, Selector};
use serde::Deserialize;
use url::Url;

use crate::freehire::{API, HOST};
use crate::{Fetcher, IngestError, check_status};

/// The largest logo read (`HttpFetcher::get_image`'s limit).
pub const MAX_LOGO_BYTES: usize = 512 * 1024;

/// Hosts (and their subdomains) whose icons are a job board's or an aggregator's, never the employer's.
const BOARDS: &[&str] = &[
    "ashbyhq.com",
    "greenhouse.io",
    "lever.co",
    "myworkdayjobs.com",
    "myworkdaysite.com",
    "workday.com",
    "icims.com",
    "applicantpro.com",
    "oraclecloud.com",
    "taleo.net",
    "smartrecruiters.com",
    "jobvite.com",
    "bamboohr.com",
    "breezy.hr",
    "recruitee.com",
    "workable.com",
    "applytojob.com",
    "jazzhr.com",
    "paylocity.com",
    "paycor.com",
    "ultipro.com",
    "ukg.net",
    "successfactors.com",
    "successfactors.eu",
    "dayforcehcm.com",
    "hrmdirect.com",
    "workforcenow.adp.com",
    "myjobs.adp.com",
    "ats.rippling.com",
    "freehire.me",
    "linkedin.com",
    "indeed.com",
    "glassdoor.com",
    "ziprecruiter.com",
    "wellfound.com",
];

/// freehire's logo for `company`, by name, as its own pages link it: 96px WebP, HTTP 404 when it has none.
pub fn freehire_logo_url(company: &str) -> String {
    let mut url = Url::parse("https://logo.freehire.me/").expect("static URL");
    url.path_segments_mut()
        .expect("an https URL has a path")
        .clear()
        .push(company.trim());
    url.into()
}

/// The company's website from freehire's company page (`GET /companies/{slug}`): `None` when freehire doesn't know
/// the company or has no website for it.
pub async fn company_website(
    fetcher: &impl Fetcher,
    slug: &str,
) -> Result<Option<String>, IngestError> {
    #[derive(Deserialize)]
    struct Page {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        company: Company,
    }
    #[derive(Deserialize)]
    struct Company {
        company_info: Option<Info>,
    }
    #[derive(Deserialize)]
    struct Info {
        website: Option<String>,
    }
    let url = company_url(slug);
    let resp = fetcher
        .get(&url)
        .await
        .map_err(|message| IngestError::Network {
            host: HOST.to_owned(),
            message,
        })?;
    if matches!(resp.status, 404 | 410) {
        return Ok(None);
    }
    check_status(&resp, HOST, &url)?;
    let page: Page = serde_json::from_str(&resp.body).map_err(|_| IngestError::Network {
        host: HOST.to_owned(),
        message: "its answer wasn't the company data it documents".to_owned(),
    })?;
    Ok(page
        .data
        .company
        .company_info
        .and_then(|i| i.website)
        .and_then(|w| site_url(&w)))
}

/// `GET /companies/{slug}`.
fn company_url(slug: &str) -> String {
    let mut url = Url::parse(API).expect("static URL");
    url.path_segments_mut()
        .expect("an https URL has a path")
        .push("companies")
        .push(slug.trim());
    url.into()
}

/// A website as written (`https://www.kla.com`, `kla.com`) as an http(s) URL.
fn site_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let url = Url::parse(raw)
        .ok()
        .filter(|u| u.has_host())
        .or_else(|| Url::parse(&format!("https://{raw}")).ok())?;
    (matches!(url.scheme(), "http" | "https") && url.host_str().is_some_and(|h| h.contains('.')))
        .then(|| url.into())
}

/// The icons a homepage links, as absolute URLs in the order to try: `apple-touch-icon` (and `-precomposed`), then
/// `icon` (`shortcut icon` too; SVG allowed), each largest `sizes` first (SVG or `any` before any size, unsized
/// last, dark-scheme ones after the rest), then `{origin}/favicon.ico`. `page_url` is the URL that answered (after
/// redirects); relative links resolve against it or the page's `<base>`. Nothing on a board's host.
pub fn icon_links(html: &str, page_url: &str) -> Vec<String> {
    static LINKS: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("link[rel][href]").expect("static selector"));
    static BASE: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("base[href]").expect("static selector"));
    let Ok(page) = Url::parse(page_url) else {
        return Vec::new();
    };
    if !is_web(&page) || is_board(&page) {
        return Vec::new();
    }
    let doc = Html::parse_document(html);
    let base = doc
        .select(&BASE)
        .find_map(|b| b.value().attr("href"))
        .and_then(|h| page.join(h.trim()).ok())
        .unwrap_or_else(|| page.clone());
    let mut touch = Vec::new();
    let mut icons = Vec::new();
    for link in doc.select(&LINKS) {
        let el = link.value();
        let (Some(rel), Some(href)) = (el.attr("rel"), el.attr("href")) else {
            continue;
        };
        let rel = rel.to_ascii_lowercase();
        let mut tokens = rel.split_ascii_whitespace();
        let list = if tokens
            .clone()
            .any(|t| matches!(t, "apple-touch-icon" | "apple-touch-icon-precomposed"))
        {
            &mut touch
        } else if tokens.any(|t| t == "icon") {
            &mut icons
        } else {
            continue;
        };
        let Ok(url) = base.join(href.trim()) else {
            continue;
        };
        let svg = el
            .attr("type")
            .is_some_and(|t| t.trim().eq_ignore_ascii_case("image/svg+xml"))
            || url.path().to_ascii_lowercase().ends_with(".svg");
        let dark = el
            .attr("media")
            .is_some_and(|m| m.to_ascii_lowercase().contains("dark"));
        list.push(((dark, Reverse(side(el.attr("sizes"), svg))), url));
    }
    // Stable: links of the same rank keep the page's order.
    touch.sort_by_key(|(rank, _)| *rank);
    icons.sort_by_key(|(rank, _)| *rank);
    let favicon = page.join("/favicon.ico").ok();
    let mut out: Vec<String> = Vec::new();
    for url in touch
        .into_iter()
        .chain(icons)
        .map(|(_, u)| u)
        .chain(favicon)
    {
        if !is_web(&url) || is_board(&url) {
            continue;
        }
        let url = String::from(url);
        if !out.contains(&url) {
            out.push(url);
        }
    }
    out
}

/// A link's `sizes` as its longest side: the largest listed, `any` (or an SVG without sizes) above all, 0 unsized.
fn side(sizes: Option<&str>, svg: bool) -> u32 {
    sizes
        .into_iter()
        .flat_map(str::split_ascii_whitespace)
        .filter_map(|s| {
            if s.eq_ignore_ascii_case("any") {
                return Some(u32::MAX);
            }
            let (w, h) = s.split_once(['x', 'X'])?;
            Some(w.parse::<u32>().ok()?.max(h.parse().ok()?))
        })
        .max()
        .unwrap_or(if svg { u32::MAX } else { 0 })
}

fn is_web(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
}

/// `url` is on a job board's host (or a subdomain of one).
fn is_board(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    BOARDS.iter().any(|b| {
        host == *b
            || host
                .strip_suffix(b)
                .is_some_and(|rest| rest.ends_with('.'))
    })
}
