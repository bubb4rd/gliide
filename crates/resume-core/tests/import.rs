use resume_core::import::{Choices, Field, master_id, merge, plan};
use resume_core::master::{Master, MasterBullet};
use resume_core::{Bank, Resume, migrate_v1, resolve};
use serde_json::json;

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");

/// Jane's bank and `swe` master, with a variant on `tidepool.1`.
fn jane() -> (Bank, Master) {
    let m = migrate_v1(&serde_json::from_str(V1).unwrap());
    let mut bank = m.bank;
    bank.sections[1].entries[0].bullets[0]
        .variants
        .push("Shipped Tidepool, a tide-chart app.".into());
    (bank, m.master)
}

fn bullet(bank: &Bank, id: &str) -> String {
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
        .find(|b| b.id == id)
        .unwrap()
        .text
        .clone()
}

/// A design resume of Jane's, as the import prompt would write it: Acme again (other dates printed, another subtitle,
/// one bullet of hers and one new), a new studio whose id the bank already uses, Tidepool through a variant, skills
/// with a known line and a new one, a section the bank has no kind for, and a portfolio link.
fn design(bank: &Bank) -> Resume {
    let acme2 = bullet(bank, "acme.2");
    serde_json::from_value(json!({
        "name": "Jane Q. Doe",
        "contact": [
            { "text": "jane.doe@example.com", "url": "mailto:jane.doe@example.com" },
            { "text": "555-010-0199", "url": "" },
            { "text": "janedoe.design", "url": "https://janedoe.design" }
        ],
        "sections": [
            { "id": "experience", "title": "Experience", "groups": [], "entries": [
                { "id": "acme", "name": "Acme Widgets Co.", "url": "", "place": "", "dates": "Jun 2025 – Present",
                  "subtitle": "Design Intern", "notes": [],
                  "bullets": [
                    { "id": "acme.1", "text": acme2.to_uppercase() },
                    { "id": "acme.2", "text": "Designed onboarding screens in Figma." }
                  ] },
                { "id": "library", "name": "Studio Nine", "url": "", "place": "Remote", "dates": "2024",
                  "subtitle": "Designer", "notes": [],
                  "bullets": [{ "id": "x", "text": "Made posters." }, { "id": "x", "text": "Made zines." }] }
            ] },
            { "id": "projects", "title": "PROJECTS", "groups": [], "entries": [
                { "id": "tidepool", "name": "Tidepool", "url": "", "place": "", "dates": "Jan 2025 – Present",
                  "subtitle": "Creator", "notes": [],
                  "bullets": [{ "id": "tidepool.1", "text": "shipped tidepool, a tide-chart app." }] }
            ] },
            { "id": "skills", "title": "SKILLS", "entries": [], "groups": [
                { "id": "tools", "label": "Tools & Technologies:", "items": ["Figma", "git", "Figma"] },
                { "id": "design", "label": "Design", "items": ["Typography", "Layout"] }
            ] },
            { "id": "volunteering", "title": "VOLUNTEERING", "groups": [], "entries": [
                { "id": "zine-fest", "name": "Zine Fest", "url": "", "place": "", "dates": "", "subtitle": "",
                  "notes": [], "bullets": [] }
            ] }
        ]
    }))
    .unwrap()
}

#[test]
fn plan_matches_sections_entries_bullets_lines_and_contact() {
    let (bank, _) = jane();
    let p = plan(Some(&bank), &design(&bank));

    let into: Vec<Option<&str>> = p.sections.iter().map(|s| s.into.as_deref()).collect();
    assert_eq!(
        into,
        [Some("work"), Some("projects"), Some("skills"), None],
        "by kind, whatever the heading; a custom section only by its heading"
    );

    let work = &p.sections[0];
    let acme = work.entries[0]
        .same
        .as_ref()
        .expect("same name, same first month");
    assert_eq!(acme.entry, "acme");
    assert_eq!(
        acme.bullets,
        [
            Some(MasterBullet {
                id: "acme.2".into(),
                variant: None
            }),
            None
        ]
    );
    assert_eq!(acme.differs, [Field::Subtitle, Field::Dates]);
    assert_eq!(work.entries[1].same, None);

    let tidepool = p.sections[1].entries[0].same.as_ref().unwrap();
    assert_eq!(
        tidepool.bullets,
        [Some(MasterBullet {
            id: "tidepool.1".into(),
            variant: Some(0)
        })]
    );
    assert!(tidepool.differs.is_empty());

    let skills = &p.sections[2].groups;
    assert_eq!(skills[0].same.as_deref(), Some("tools"));
    assert_eq!(skills[0].new_items, ["Figma"]);
    assert_eq!(skills[1].same, None);
    assert_eq!(skills[1].new_items, ["Typography", "Layout"]);

    let contact: Vec<&str> = p.contact.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(
        contact,
        ["janedoe.design"],
        "the phone number matches by its digits"
    );

    // Ids unique within the file; bullets numbered by entry.
    let studio = &p.incoming.sections[0].entries[1];
    let ids: Vec<&str> = studio.bullets.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(
        ids,
        [format!("{}.1", studio.id), format!("{}.2", studio.id)]
    );
}

#[test]
fn merge_adds_only_whats_new_and_keeps_every_master() {
    let (bank, swe) = jane();
    let before = resolve(&bank, &swe).unwrap();
    let file = design(&bank);
    let p = plan(Some(&bank), &file);
    let m = merge(Some(&bank), &p, &Choices::default());

    // Every existing master resolves as it did; nothing of the bank changed but additions.
    assert_eq!(resolve(&m.bank, &swe).unwrap(), before);
    assert_eq!(m.bank.name, bank.name);
    assert_eq!(
        m.bank.contact, bank.contact,
        "contact items are only offered"
    );

    let master = Master {
        id: "design".into(),
        title: "Design".into(),
        template: "classic-serif".into(),
        page_limit: 1,
        sections: m.sections,
    };
    let r = resolve(&m.bank, &master).unwrap();
    let titles: Vec<&str> = r.sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Experience", "PROJECTS", "SKILLS", "VOLUNTEERING"]);

    // Acme is the bank's entry, showing the file's bullets in its order: Jane's own, then the new one.
    let acme = &r.sections[0].entries[0];
    assert_eq!(
        (acme.id.as_str(), acme.subtitle.as_str()),
        ("acme", "Software Engineering Intern")
    );
    let texts: Vec<&str> = acme.bullets.iter().map(|b| b.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            bullet(&bank, "acme.2").as_str(),
            "Designed onboarding screens in Figma."
        ]
    );
    assert_eq!(acme.bullets[1].id, "acme.4");

    // The studio took a free id: the bank's `library` is another entry.
    let studio = &r.sections[0].entries[1];
    assert_eq!(studio.id, "library-2");
    let ids: Vec<&str> = studio.bullets.iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, ["library-2.1", "library-2.2"]);

    // Tidepool through its variant.
    assert_eq!(
        r.sections[1].entries[0].bullets[0].text,
        "Shipped Tidepool, a tide-chart app."
    );

    // Skills in the bank's spelling, in the file's order; the bank line grew by the new item only.
    let lines: Vec<(&str, Vec<&str>)> = r.sections[2]
        .groups
        .iter()
        .map(|g| {
            (
                g.label.as_str(),
                g.items.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            ("Tools & Technologies", vec!["Figma", "Git"]),
            ("Design", vec!["Typography", "Layout"])
        ]
    );
    let tools = &m.bank.sections[3].groups[1];
    assert_eq!(
        tools.items.len(),
        bank.sections[3].groups[1].items.len() + 1
    );
    assert_eq!(tools.items.last().map(String::as_str), Some("Figma"));

    assert_eq!(r.sections[3].entries[0].name, "Zine Fest");
}

#[test]
fn kept_separate_and_chosen_contact() {
    let (bank, swe) = jane();
    let p = plan(Some(&bank), &design(&bank));
    let choices = Choices {
        separate: ["acme".to_owned()].into(),
        contact: [0].into(),
    };
    let m = merge(Some(&bank), &p, &choices);
    // A chosen contact item heads every resume; the rest of each master is as it was.
    let (now, was) = (
        resolve(&m.bank, &swe).unwrap(),
        resolve(&bank, &swe).unwrap(),
    );
    assert_eq!(now.sections, was.sections);
    assert_eq!(now.contact.len(), was.contact.len() + 1);
    assert_eq!(m.bank.contact.last().unwrap().url, "https://janedoe.design");

    let work = &m.bank.sections[0];
    let acme = work.entries.iter().find(|e| e.id == "acme").unwrap();
    assert_eq!(acme.bullets.len(), 3, "your Acme is untouched");
    let own = work
        .entries
        .iter()
        .find(|e| e.id == "acme-2")
        .expect("an entry of its own");
    assert_eq!(own.subtitle, "Design Intern");
    assert_eq!(own.start.as_deref(), Some("2025-06"));
    assert_eq!(own.end.as_deref(), Some("present"));
    assert_eq!(m.sections[0].entries[0].id, "acme-2");
}

#[test]
fn first_import_is_the_file() {
    let file: Resume = serde_json::from_str(V1).unwrap();
    let p = plan(None, &file);
    assert!(p.sections.iter().all(|s| s.into.is_none()));
    assert!(p.contact.is_empty());
    let m = merge(None, &p, &Choices::default());
    let master = Master {
        id: "swe".into(),
        title: "Software Engineering".into(),
        template: "classic-serif".into(),
        page_limit: 1,
        sections: m.sections,
    };
    let migrated = migrate_v1(&file);
    assert_eq!(m.bank, migrated.bank);
    assert_eq!(master, migrated.master);
}

#[test]
fn entries_match_by_name_and_first_or_last_month() {
    let (bank, _) = jane();
    let mut file: Resume = serde_json::from_str(V1).unwrap();
    // A longer name that starts with the bank's, only the graduation month printed: the same school.
    file.sections[2].entries[0].name = "State University, College of Engineering".into();
    file.sections[2].entries[0].dates = "Expected May 2026".into();
    // Other first and last months: another stint.
    file.sections[0].entries[0].dates = "Jan 2026 – Present".into();
    // A name that only starts with the same letters isn't the same.
    file.sections[1].entries[0].name = "Tidepools".into();
    let p = plan(Some(&bank), &file);
    assert_eq!(
        p.sections[2].entries[0]
            .same
            .as_ref()
            .map(|s| s.entry.as_str()),
        Some("state-u")
    );
    assert_eq!(p.sections[0].entries[0].same, None);
    assert_eq!(p.sections[1].entries[0].same, None);
}

#[test]
fn master_ids_are_free_slugs() {
    assert_eq!(master_id("Graphic Design", &["swe"]), "graphic-design");
    assert_eq!(master_id("SWE", &["swe"]), "swe-2");
    assert_eq!(master_id("  ", &[]), "master");
}
