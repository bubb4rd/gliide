use resume_core::{Resume, migrate_v1, resolve};
use resume_render::{Candidate, FontOptions, LineKind, Renderer, Template};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");

fn fixture() -> Resume {
    let m = migrate_v1(&serde_json::from_str(V1).unwrap());
    resolve(&m.bank, &m.master).unwrap()
}

fn renderer() -> Renderer {
    Renderer::new(&Template::classic_serif(), FontOptions::default())
}

#[test]
fn fixture_renders_one_page_with_a_metric_per_bullet_and_group() {
    let resume = fixture();
    let out = renderer()
        .render(&serde_json::to_string(&resume).unwrap(), false)
        .unwrap();
    // Bundled fonts only: Times New Roman is missing and Liberation Serif, next in the template's list, is used.
    assert_eq!(
        out.warnings,
        ["warning: unknown font family: times new roman (resume.typ:22)"]
    );
    assert_eq!(out.metrics.pages, 1);
    assert_eq!(out.page_count(), 1);

    let bullets: Vec<_> = resume
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
        .map(|b| &b.id)
        .collect();
    let groups: Vec<_> = resume
        .sections
        .iter()
        .flat_map(|s| &s.groups)
        .map(|g| &g.id)
        .collect();
    assert_eq!(
        out.metrics
            .bullets
            .iter()
            .map(|b| &b.id)
            .collect::<Vec<_>>(),
        bullets
    );
    assert_eq!(
        out.metrics.groups.iter().map(|g| &g.id).collect::<Vec<_>>(),
        groups
    );
    for m in out.metrics.bullets.iter().chain(&out.metrics.groups) {
        assert!(m.lines >= 1, "{m:?}");
        assert_eq!(
            m.orig, m.lines,
            "no orig text, so orig is its own count: {m:?}"
        );
    }
    // The long first bullet wraps; the short Languages line doesn't.
    assert_eq!(out.metrics.bullets[0].lines, 2);
    assert_eq!(out.metrics.groups[0].lines, 1);

    assert!(out.pdf().unwrap().starts_with(b"%PDF-"));
    let svgs = out.svgs();
    assert_eq!(svgs.len(), 1);
    assert!(svgs[0].contains("<svg"));
    let pngs = out.pngs(1.0).unwrap();
    assert_eq!(pngs.len(), 1);
    assert!(pngs[0].starts_with(b"\x89PNG"));
}

#[test]
fn orig_counts_the_text_a_change_replaces() {
    let mut resume = fixture();
    let bullet = &mut resume.sections[0].entries[0].bullets[2];
    bullet.orig = Some(format!("{} {}", bullet.text, bullet.text.repeat(3)));
    bullet.changed = true;
    let group = &mut resume.sections[3].groups[0];
    group.orig = Some(group.items.clone());
    group.items.push("Kotlin".into());
    group.added = vec!["Kotlin".into()];

    let out = renderer()
        .render(&serde_json::to_string(&resume).unwrap(), true)
        .unwrap();
    let b = out
        .metrics
        .bullets
        .iter()
        .find(|b| b.id == "acme.3")
        .unwrap();
    assert_eq!(b.lines, 1);
    assert!(b.orig > b.lines, "{b:?}");
    let g = &out.metrics.groups[0];
    assert_eq!((g.lines, g.orig), (1, 1));
}

#[test]
fn rerender_after_an_edit_reflects_it() {
    let mut resume = fixture();
    let mut r = renderer();
    let before = r
        .render(&serde_json::to_string(&resume).unwrap(), false)
        .unwrap();
    resume.sections[0].entries[0].bullets[2]
        .text
        .push_str(&" and more".repeat(20));
    let after = r
        .render(&serde_json::to_string(&resume).unwrap(), false)
        .unwrap();
    let lines =
        |m: &resume_render::Metrics| m.bullets.iter().find(|b| b.id == "acme.3").unwrap().lines;
    assert!(lines(&after.metrics) > lines(&before.metrics));
}

#[test]
fn bad_data_is_a_compile_error() {
    let err = renderer().render("{not json", false).err().expect("fails");
    assert!(err.to_string().contains("error"), "{err}");
}

/// Every bullet and skills line of the fixture as a measure candidate: same text, same line.
fn candidates(resume: &Resume) -> Vec<Candidate> {
    let mut out = Vec::new();
    for s in &resume.sections {
        for e in &s.entries {
            let depth = u32::from(!e.notes.is_empty());
            for b in &e.bullets {
                out.push(Candidate {
                    id: b.id.clone(),
                    kind: LineKind::Bullet,
                    depth,
                    text: b.text.clone(),
                    label: String::new(),
                });
            }
        }
        for g in &s.groups {
            out.push(Candidate {
                id: g.id.clone(),
                kind: LineKind::Group,
                depth: 0,
                text: g.items.join(", "),
                label: g.label.clone(),
            });
        }
    }
    out
}

#[test]
fn measure_counts_each_line_as_the_render_does() {
    let resume = fixture();
    let mut r = renderer();
    let rendered = r
        .render(&serde_json::to_string(&resume).unwrap(), false)
        .unwrap();
    let cands = candidates(&resume);
    let measured = r.measure(&cands).unwrap();
    let rendered: Vec<_> = rendered
        .metrics
        .bullets
        .iter()
        .chain(&rendered.metrics.groups)
        .map(|m| (m.id.clone(), m.lines))
        .collect();
    assert_eq!(measured, rendered);
    // Some bullet sits under an entry with notes, so both widths are exercised.
    assert!(cands.iter().any(|c| c.depth == 1), "{cands:?}");
}

#[test]
fn measure_counts_a_longer_text_as_more_lines() {
    let resume = fixture();
    let b = &resume.sections[0].entries[0].bullets[2];
    let note = |text: String| Candidate {
        id: "n".into(),
        kind: LineKind::Note,
        depth: 0,
        text,
        label: "Coursework".into(),
    };
    let bullet = |id: &str, text: String| Candidate {
        id: id.into(),
        kind: LineKind::Bullet,
        depth: 0,
        text,
        label: String::new(),
    };
    let measured = renderer()
        .measure(&[
            bullet("short", b.text.clone()),
            bullet("long", format!("{} {}", b.text, "and more".repeat(30))),
            note("Data Structures".into()),
            note("Data Structures, ".repeat(20)),
        ])
        .unwrap();
    let lines: Vec<u32> = measured.iter().map(|m| m.1).collect();
    assert_eq!(lines[0], 1);
    assert!(lines[1] > lines[0], "{measured:?}");
    assert_eq!(lines[2], 1);
    assert!(lines[3] > 1, "{measured:?}");
    assert!(renderer().measure(&[]).unwrap().is_empty());
}
