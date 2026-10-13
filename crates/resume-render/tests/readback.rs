//! The PDF read-back through the score's format check: the text a parser reads from the exported PDF carries every
//! matched skill, unless the page breaks one. And the import's read of the same PDF: that text plus its links.

use resume_core::{
    PageFacts, Requirements, Resume, Score, migrate_v1, resolve, resume_text, score,
};
use resume_render::{FontOptions, Link, Renderer, Template, pdf_read};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");
const REQ: &str = include_str!("../../../fixtures/score/jane-doe-swe.requirements.json");
const TODAY: (u32, u32) = (2026, 10);

fn jane() -> Resume {
    let m = migrate_v1(&serde_json::from_str(V1).unwrap());
    resolve(&m.bank, &m.master).unwrap()
}

/// The fixture's requirements plus `front-end`, a hard skill.
fn req() -> Requirements {
    let mut req: serde_json::Value = serde_json::from_str(REQ).unwrap();
    req["skills"].as_array_mut().unwrap().push(serde_json::json!({
        "term": "front-end", "aliases": [], "kind": "hard", "priority": 2, "required": false, "group": null
    }));
    serde_json::from_value(req).unwrap()
}

fn scored(resume: &Resume, pdf_text: String) -> Score {
    let facts = PageFacts {
        pages: 1,
        page_limit: 1,
        ats_safe: true,
        pdf_text: Some(pdf_text),
    };
    score(resume, &req(), &facts, TODAY)
}

fn format_detail(s: &Score) -> &str {
    &s.components
        .iter()
        .find(|c| c.id == "format")
        .unwrap()
        .detail
}

/// Whitespace runs as one space, as the matcher reads text.
fn words(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Jane with `front-end` in her first bullet, preceded by `pad` filler words.
fn with_front_end(pad: usize) -> Resume {
    let mut r = jane();
    let b = &mut r.sections[0].entries[0].bullets[0];
    b.text = format!(
        "Worked closely with the team{} and built the front-end",
        " daily".repeat(pad)
    );
    r
}

#[test]
fn the_pdf_reads_back_the_page_and_passes_the_check() {
    let r = with_front_end(0);
    let mut renderer = Renderer::new(&Template::classic_serif(), FontOptions::default());
    let rendered = renderer
        .render(&serde_json::to_string(&r).unwrap(), false)
        .unwrap();
    let text = rendered.pdf_text().unwrap();
    // Every bullet and skills line reads back as printed, wraps aside.
    let pdf = words(&text);
    for s in &r.sections {
        for b in s.entries.iter().flat_map(|e| &e.bullets) {
            assert!(pdf.contains(&words(&b.text)), "{:?} not in {pdf}", b.text);
        }
        for g in &s.groups {
            assert!(
                pdf.contains(&g.items.join(", ")),
                "{:?} not in {pdf}",
                g.items
            );
        }
    }
    // The renderer's read-back is the same text.
    assert_eq!(renderer.pdf_text(&rendered).unwrap(), text);

    let s = scored(&r, text);
    assert!(
        format_detail(&s).ends_with("PDF text carries every matched skill"),
        "{}",
        format_detail(&s)
    );
    // Identical text scores as the page's own text does.
    assert_eq!(s, scored(&r, resume_text(&r)));
}

#[test]
fn a_term_a_line_break_splits_fails_the_check() {
    let mut renderer = Renderer::new(&Template::classic_serif(), FontOptions::default());
    // Pad the bullet until its first line ends at `front-`.
    let (r, text) = (0..60)
        .find_map(|pad| {
            let r = with_front_end(pad);
            let text = renderer
                .render(&serde_json::to_string(&r).unwrap(), false)
                .unwrap()
                .pdf_text()
                .unwrap();
            text.contains("front-\nend").then_some((r, text))
        })
        .expect("some padding breaks the line after `front-`");

    // The page's own text has the term whole; the PDF's doesn't.
    let own = scored(&r, resume_text(&r));
    assert!(
        own.skills
            .iter()
            .any(|m| m.term == "front-end" && m.status.literal())
    );
    let s = scored(&r, text);
    assert!(
        format_detail(&s).ends_with("PDF text lacks: front-end"),
        "{}",
        format_detail(&s)
    );
    assert!(s.total < own.total, "{} < {}", s.total, own.total);
}

#[test]
fn the_import_reads_the_pdf_text_and_the_links_under_their_text() {
    let r = jane();
    let pdf = Renderer::new(&Template::classic_serif(), FontOptions::default())
        .render(&serde_json::to_string(&r).unwrap(), false)
        .unwrap()
        .pdf()
        .unwrap();
    let read = pdf_read(pdf.clone()).unwrap();
    assert_eq!(read.text, resume_render::pdf_text(pdf).unwrap());
    assert_eq!(read.pages, 1);
    // classic-serif links contact items (not `mailto:`) and entry names that have a URL.
    let link = |text: &str, url: &str| Link {
        text: text.into(),
        url: url.into(),
    };
    assert_eq!(
        read.links,
        [
            link(
                "linkedin.com/in/janeqdoe-example",
                "https://linkedin.com/in/janeqdoe-example"
            ),
            link(
                "github.com/janeqdoe-example",
                "https://github.com/janeqdoe-example"
            ),
            link("Acme Widgets Co.", "https://example.com/acme"),
            link("Tidepool", "https://github.com/janeqdoe-example/tidepool"),
        ]
    );
}
