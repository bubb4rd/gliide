use resume_core::bank::{BankEntry, Note};
use resume_core::details::{Details, DetailsError, add_entry, set_details};
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

fn details_of(e: &BankEntry) -> Details {
    Details {
        name: e.name.clone(),
        subtitle: e.subtitle.clone(),
        place: e.place.clone(),
        dates: e.dates.clone(),
        notes: e.notes.clone(),
    }
}

#[test]
fn new_details_print_in_every_master_and_lines_go() {
    let (mut bank, master) = jane();
    let mut details = details_of(entry(&bank, "state-u"));
    details.subtitle = "  Bachelor of Science in Computer Science Engineering ".into();
    details.notes.remove(0);
    details.notes[0].label = " Honors ".into();
    set_details(&mut bank, "state-u", details).unwrap();

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
    let mut details = details_of(entry(&bank, "acme"));
    bank.sections[0].entries[0].start = Some("2025-05".into());
    set_details(&mut bank, "acme", details.clone()).unwrap();
    assert_eq!(entry(&bank, "acme").start.as_deref(), Some("2025-05"));

    details.dates = "May 2025 – Present".into();
    set_details(&mut bank, "acme", details.clone()).unwrap();
    let e = entry(&bank, "acme");
    assert_eq!(
        (e.start.as_deref(), e.end.as_deref()),
        (Some("2025-05"), Some("present"))
    );

    details.dates = "Summer 2025".into();
    set_details(&mut bank, "acme", details).unwrap();
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
    let details = details_of(entry(&bank, "state-u"));

    let nameless = Details {
        name: "  ".into(),
        ..details.clone()
    };
    assert_eq!(
        set_details(&mut bank, "state-u", nameless),
        Err(DetailsError::Name)
    );

    let mut blank_line = details.clone();
    blank_line.notes.push(Note {
        label: "Minor".into(),
        text: " ".into(),
    });
    assert_eq!(
        set_details(&mut bank, "state-u", blank_line),
        Err(DetailsError::Note)
    );

    assert_eq!(
        set_details(&mut bank, "nowhere", details),
        Err(DetailsError::Entry("nowhere".into()))
    );
    assert_eq!(bank, before, "a refused edit changes nothing");
}

#[test]
fn a_new_entry_ends_its_section_in_no_master_under_an_id_of_its_own() {
    let (mut bank, master) = jane();
    let page = resolve(&bank, &master).unwrap();

    let id = add_entry(&mut bank, "projects", "  Tidepool  ").unwrap();
    assert_eq!(id, "tidepool-2", "the fixture's project holds `tidepool`");
    let again = add_entry(&mut bank, "work", "Tidepool").unwrap();
    assert_eq!(again, "tidepool-3");

    let projects = bank.sections.iter().find(|s| s.id == "projects").unwrap();
    let last = projects.entries.last().unwrap();
    assert_eq!(
        (last.id.as_str(), last.name.as_str()),
        ("tidepool-2", "Tidepool")
    );
    assert!(last.bullets.is_empty() && last.start.is_none());
    assert_eq!(resolve(&bank, &master).unwrap(), page, "no master shows it");
}

#[test]
fn a_new_entry_needs_a_name_and_a_section_that_holds_entries() {
    let (mut bank, _) = jane();
    let before = bank.clone();
    assert_eq!(add_entry(&mut bank, "work", " "), Err(DetailsError::Name));
    assert_eq!(
        add_entry(&mut bank, "skills", "Rust"),
        Err(DetailsError::Section("skills".into()))
    );
    assert_eq!(
        add_entry(&mut bank, "nowhere", "Rust"),
        Err(DetailsError::Section("nowhere".into()))
    );
    assert_eq!(bank, before, "a refused entry changes nothing");
}
