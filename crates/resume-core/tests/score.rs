//! Golden score: the synthetic Jane Doe resume against hand-written requirements, every component worked out by hand.

use resume_core::matcher::Status;
use resume_core::score::Component;
use resume_core::{
    PageFacts, Requirements, Resume, Score, migrate_v1, rank, resolve, resume_text, score,
};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");
const REQ: &str = include_str!("../../../fixtures/score/jane-doe-swe.requirements.json");
const TODAY: (u32, u32) = (2026, 10);

fn jane() -> Resume {
    let m = migrate_v1(&serde_json::from_str(V1).unwrap());
    resolve(&m.bank, &m.master).unwrap()
}

fn req() -> Requirements {
    serde_json::from_str(REQ).unwrap()
}

fn facts() -> PageFacts {
    PageFacts {
        pages: 1,
        page_limit: 1,
        ats_safe: true,
        pdf_text: None,
    }
}

fn component<'a>(s: &'a Score, id: &str) -> &'a Component {
    s.components.iter().find(|c| c.id == id).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn resume_text_reads_in_page_order() {
    let text = resume_text(&jane());
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "Jane Q. Doe");
    assert!(lines[1].starts_with("jane.doe@example.com | (555) 010-0199 | "));
    assert_eq!(
        &lines[2..6],
        [
            "WORK EXPERIENCE",
            "Acme Widgets Co., Springfield, IL",
            "June 2025 – Sep 2025",
            "Software Engineering Intern"
        ]
    );
    assert!(text.contains("\nRelevant Coursework: Data Structures, Operating Systems"));
    assert!(text.ends_with(
        "SKILLS\nLanguages: Rust, TypeScript, Python, SQL, Java\nTools & Technologies: Git, GitHub Actions, Docker, \
         PostgreSQL, SQLite, React, Linux, AWS"
    ));
}

#[test]
fn golden_jane_doe() {
    let s = score(&jane(), &req(), &facts(), TODAY);

    // Skills, by hand (weight 3/2/1 by priority, ×1.5 required):
    let status: Vec<(&str, Status, Option<&str>, Option<&str>)> = s
        .skills
        .iter()
        .map(|m| {
            (
                m.term.as_str(),
                m.status,
                m.text.as_deref(),
                m.section.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        status,
        [
            // p1 required, 4.5: "Built a Rust service…".
            ("Rust", Status::Exact, Some("Rust"), Some("work")),
            // p2, 2: only "Java" and "TypeScript" are on the page, and Java is not JavaScript.
            ("JavaScript", Status::Missing, None, None),
            // p2 required, 3: the dictionary's alias, on the Tools line.
            (
                "Amazon Web Services",
                Status::Alias,
                Some("AWS"),
                Some("skills")
            ),
            // p2, 2: the page says "integration tests", a near form only.
            (
                "Integration test",
                Status::Near,
                Some("integration tests"),
                Some("work")
            ),
            // One-of group `frontend` (both p2 required, 3): met by React.
            ("React", Status::Exact, Some("React"), Some("work")),
            ("Vue", Status::Missing, None, None),
            // One-of group `nosql` (both p3, 1): neither.
            ("MongoDB", Status::Missing, None, None),
            ("DynamoDB", Status::Missing, None, None),
            // p3, 1.
            ("Docker", Status::Exact, Some("Docker"), Some("skills")),
            // p3, 1: no whole-term "C" anywhere (not in "Co.", "City", "Computer").
            ("C", Status::Missing, None, None),
            // Domain p2, 2: "batches widget telemetry".
            ("Telemetry", Status::Exact, Some("telemetry"), Some("work")),
            // Domain p3, 1: "help-desk tickets", a near form.
            ("Help desk", Status::Near, Some("help-desk"), Some("work")),
            // Soft p3, 1.
            ("Mentoring", Status::Missing, None, None),
        ]
    );

    // Hard + tool: 8 requirements (two groups count once), weights 4.5 + 2 + 3 + 2 + 3 + 1 + 1 + 1 = 17.5.
    // Met: Rust 4.5, AWS 3, frontend 3, Docker 1 = 11.5. With near forms + integration test 2 = 13.5.
    let c = component(&s, "hard_tool");
    assert!(c.present);
    assert!(close(c.value, 11.5 / 17.5), "{c:?}");
    assert!(close(c.value_near, 13.5 / 17.5), "{c:?}");
    assert_eq!(
        c.detail,
        "4 of 8 on the page (11.5 of 17.5 weight); missing: JavaScript, Integration test, MongoDB or DynamoDB, C; \
         near forms: Integration test \u{2248} \"integration tests\""
    );

    // Domain + soft: Telemetry 2, help desk 1 (near), mentoring 1 = 4. Met 2; with near forms 3.
    let c = component(&s, "domain_soft");
    assert!(close(c.value, 0.5) && close(c.value_near, 0.75), "{c:?}");

    // Title: subtitles are "Software Engineering Intern", "IT Assistant", "Creator", "B.S. Computer Science".
    // "Software Engineer" is not whole in "Software Engineering", but its word "Software" is: partial, 0.5.
    // "Backend Engineer": neither word whole. Best 0.5.
    let c = component(&s, "title");
    assert!(close(c.value, 0.5), "{c:?}");
    assert_eq!(
        c.detail,
        "in a subtitle or summary: \"Software Engineer\" partly stated: \"Software\"; \"Backend Engineer\" not stated"
    );

    // Experience: the work section has Acme (Jun–Sep 2025, 4 months) and the library ("Summer 2023", no months).
    // Tidepool runs to present but is a project, not experience. 4 months of 12 needed: 1/3.
    let c = component(&s, "experience");
    assert!(close(c.value, 4.0 / 12.0), "{c:?}");
    assert_eq!(
        c.detail,
        "0.3 years (4 months, overlaps once) from 1 dated entry in experience sections vs 1 required; 1 entry \
         without readable dates not counted"
    );

    // Education: "B.S. Computer Science" is a bachelor's in a required field: degree part 1. The AWS certification
    // isn't on the page: certifications part 0. Mean 0.5.
    let c = component(&s, "education");
    assert!(close(c.value, 0.5), "{c:?}");
    assert_eq!(
        c.detail,
        "degree 1: \"B.S. Computer Science\" (bachelor) meets bachelor; field Computer Science stated: \"Computer \
         Science\"; certifications 0 of 1; missing: AWS Certified Cloud Practitioner"
    );

    // Format: ATS-safe template, standard headings (WORK EXPERIENCE, PROJECTS, EDUCATION, SKILLS), a mailto email, a
    // 10-digit phone, 1 of 1 pages, no PDF to read back: 5 of 5.
    let c = component(&s, "format");
    assert!(close(c.value, 1.0), "{c:?}");

    // Total: (45 × 11.5/17.5 + 10 × 0.5 + 10 × 0.5 + 15 × 1/3 + 10 × 0.5 + 10 × 1) / 100
    //      = (29.571 + 5 + 5 + 5 + 5 + 10) / 100 = 59.571 → 59.6.
    assert_eq!(s.total, 59.6);
    // With near forms: (45 × 13.5/17.5 + 10 × 0.75 + 5 + 5 + 5 + 10) / 100 = 67.214 → 67.2.
    assert_eq!(s.total_near, 67.2);

    assert_eq!(s.knockouts.len(), 1);
    assert_eq!(
        s.knockouts[0].text,
        "On-site in Springfield, IL three days a week"
    );
}

#[test]
fn golden_renormalizes_without_years_or_education() {
    let mut req = req();
    req.years_min = None;
    req.degree = None;
    req.certifications.clear();
    let s = score(&jane(), &req, &facts(), TODAY);
    assert!(!component(&s, "experience").present);
    assert!(!component(&s, "education").present);
    // (29.571 + 5 + 5 + 10) / 75 = 66.095 → 66.1; with near forms (34.714 + 7.5 + 5 + 10) / 75 = 76.286 → 76.3.
    assert_eq!(s.total, 66.1);
    assert_eq!(s.total_near, 76.3);
}

#[test]
fn golden_pdf_read_back() {
    let jane = jane();
    // The PDF's text lost the Tools line's "Docker".
    let pdf = resume_text(&jane).replace("Docker, ", "");
    let facts = PageFacts {
        pdf_text: Some(pdf),
        ..facts()
    };
    let s = score(&jane, &req(), &facts, TODAY);
    let c = component(&s, "format");
    assert!(close(c.value, 5.0 / 6.0), "{c:?}");
    assert!(c.detail.ends_with("PDF text lacks: Docker"), "{}", c.detail);
    // 59.571 − 10 × 1/6 = 57.905 → 57.9.
    assert_eq!(s.total, 57.9);
    // The page's own text reads back whole.
    let facts = PageFacts {
        pdf_text: Some(resume_text(&jane)),
        ..facts
    };
    assert_eq!(score(&jane, &req(), &facts, TODAY).total, 59.6);
}

#[test]
fn rank_puts_the_best_master_first() {
    let full = jane();
    let mut lean = full.clone();
    lean.sections.retain(|s| s.id != "skills");
    let ranked = rank(
        &[("lean", lean, facts()), ("swe", full, facts())],
        &req(),
        TODAY,
    );
    assert_eq!(ranked[0], ("swe", 59.6));
    assert_eq!(ranked[1].0, "lean");
    assert!(ranked[1].1 < 59.6);
}
