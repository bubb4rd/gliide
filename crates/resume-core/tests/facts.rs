use resume_core::bank::{BankEntry, Note};
use resume_core::facts::{Facts, FactsError, set_facts};
use resume_core::{Bank, Resume, migrate_v1, resolve};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");

fn jane() -> (Bank, resume_core::Master) {
    let m = migrate_v1(&serde_json::from_str::<Resume>(V1).unwrap());
    (m.bank, m.master)
}

fn entry<'a>(bank: &'a Bank, id: &str) -> &'a BankEntry {
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == id)
        .unwrap()
}

fn facts_of(e: &BankEntry) -> Facts {
    Facts {
        name: e.name.clone(),
        subtitle: e.subtitle.clone(),
        place: e.place.clone(),
        dates: e.dates.clone(),
        notes: e.notes.clone(),
    }
}

#[test]
fn new_facts_print_in_every_master_and_lines_go() {
    let (mut bank, master) = jane();
    let mut facts = facts_of(entry(&bank, "state-u"));
    facts.subtitle = "  Bachelor of Science in Computer Science Engineering ".into();
    facts.notes.remove(0);
    facts.notes[0].label = " Honors ".into();
    set_facts(&mut bank, "state-u", facts).unwrap();

    let page = resolve(&bank, &master).unwrap();
    let school = page
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == "state-u")
        .unwrap();
    assert_eq!(
        school.subtitle,
        "Bachelor of Science in Computer Science Engineering"
    );
    assert_eq!(
        school
            .notes
            .iter()
            .map(|n| (n.label.as_str(), n.text.as_str()))
            .collect::<Vec<_>>(),
        [("Honors", "Dean's List (6 semesters)")]
    );
}

#[test]
fn new_dates_are_read_again_and_the_same_dates_keep_their_months() {
    let (mut bank, _) = jane();
    // The same printed dates leave the months alone, even ones read some other way.
    let mut facts = facts_of(entry(&bank, "acme"));
    bank.sections[0].entries[0].start = Some("2025-05".into());
    set_facts(&mut bank, "acme", facts.clone()).unwrap();
    assert_eq!(entry(&bank, "acme").start.as_deref(), Some("2025-05"));

    facts.dates = "May 2025 – Present".into();
    set_facts(&mut bank, "acme", facts.clone()).unwrap();
    let e = entry(&bank, "acme");
    assert_eq!(
        (e.start.as_deref(), e.end.as_deref()),
        (Some("2025-05"), Some("present"))
    );

    facts.dates = "Summer 2025".into();
    set_facts(&mut bank, "acme", facts).unwrap();
    let e = entry(&bank, "acme");
    assert_eq!(
        (e.dates.as_str(), e.start.as_deref(), e.end.as_deref()),
        ("Summer 2025", None, None)
    );
}

#[test]
fn a_name_and_every_line_need_text() {
    let (mut bank, _) = jane();
    let before = bank.clone();
    let facts = facts_of(entry(&bank, "state-u"));

    let nameless = Facts {
        name: "  ".into(),
        ..facts.clone()
    };
    assert_eq!(
        set_facts(&mut bank, "state-u", nameless),
        Err(FactsError::Name)
    );

    let mut blank_line = facts.clone();
    blank_line.notes.push(Note {
        label: "Minor".into(),
        text: " ".into(),
    });
    assert_eq!(
        set_facts(&mut bank, "state-u", blank_line),
        Err(FactsError::Note)
    );

    assert_eq!(
        set_facts(&mut bank, "nowhere", facts),
        Err(FactsError::Entry("nowhere".into()))
    );
    assert_eq!(bank, before, "a refused edit changes nothing");
}
