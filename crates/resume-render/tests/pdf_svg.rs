//! A PDF previewed as SVG (the export folder's files): one document per page, each drawing only from its own
//! definitions when the pages share one HTML document, with glyph outlines shared across a page.

use std::collections::HashSet;

use resume_core::{migrate_v1, resolve};
use resume_render::{FontOptions, Renderer, Template, pdf_svgs};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");

/// Every value in `svg` that follows `site` up to `end`.
fn after<'a>(svg: &'a str, site: &str, end: char) -> Vec<&'a str> {
    svg.split(site)
        .skip(1)
        .map(|s| &s[..s.find(end).unwrap()])
        .collect()
}

#[test]
fn a_two_page_pdf_previews_as_two_pages_that_reference_only_their_own_definitions() {
    let m = migrate_v1(&serde_json::from_str(V1).unwrap());
    let mut resume = resolve(&m.bank, &m.master).unwrap();
    let again = resume.sections.clone();
    resume.sections.extend(again.clone());
    resume.sections.extend(again);
    let pdf = Renderer::new(&Template::classic_serif(), FontOptions::default())
        .render(&serde_json::to_string(&resume).unwrap(), false)
        .unwrap()
        .pdf()
        .unwrap();

    let pages = pdf_svgs(pdf).unwrap();
    assert_eq!(pages.len(), 2);
    let mut seen = HashSet::new();
    for page in &pages {
        let ids: HashSet<&str> = after(page, " id=\"", '"').into_iter().collect();
        for id in &ids {
            assert!(seen.insert(id.to_string()), "{id} is defined on two pages");
        }
        let refs = after(page, "href=\"#", '"')
            .into_iter()
            .chain(after(page, "url(#", ')'));
        for r in refs {
            assert!(ids.contains(r), "#{r} isn't defined on its page");
        }
        let uses = page.matches("<use ").count();
        let outlines = after(page, "<path id=\"", '"').len();
        assert!(outlines * 4 < uses, "{outlines} outlines for {uses} glyphs");
    }
}

#[test]
fn anything_but_a_pdf_has_no_pages() {
    assert_eq!(pdf_svgs(b"not a pdf".to_vec()), None);
}
