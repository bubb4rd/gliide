//! Facts: what you've said about an entry that its lines don't print.

use resume_core::bank::{BankEntry, Fact, FactKind, FactSource};
use resume_core::facts::{FactDraft, FactError, add_fact, set_facts};
use resume_core::{Bank, Resume, migrate_v1, resolve};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");

fn jane() -> (Bank, resume_core::Master) {
    let m = migrate_v1(&serde_json::from_str::<Resume>(V1).unwrap());
    (m.bank, m.master)
}

fn source(kind: FactKind, at: &str) -> FactSource {
    FactSource {
        kind,
        at: at.into(),
        posting: None,
        company: None,
    }
}

fn acme(bank: &Bank) -> &BankEntry {
    bank.sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == "acme")
        .unwrap()
}

fn draft(f: &Fact) -> FactDraft {
    FactDraft {
        id: Some(f.id.clone()),
        text: f.text.clone(),
        skills: f.skills.clone(),
        source: Some(f.source.clone()),
    }
}

fn new_draft(text: &str) -> FactDraft {
    FactDraft {
        id: None,
        text: text.into(),
        skills: vec![],
        source: None,
    }
}

#[test]
fn facts_never_reach_the_page() {
    let (mut bank, master) = jane();
    add_fact(
        &mut bank,
        "acme",
        "The order API handled about 2,000 orders a day",
        &["Kafka".into()],
        source(FactKind::Edit, "2026-10-27T10:00:00Z"),
    )
    .unwrap();
    let page = serde_json::to_string(&resolve(&bank, &master).unwrap()).unwrap();
    assert!(!page.contains("2,000 orders"), "{page}");
    assert!(!page.contains("Kafka"), "{page}");
}

#[test]
fn a_bank_without_facts_reads_and_writes_as_before() {
    // Revisions are keyed by the JSON's hash: a bank with no facts must serialize exactly as it did before facts.
    let (bank, _) = jane();
    let json = serde_json::to_string(&bank).unwrap();
    assert!(!json.contains("\"facts\""), "{json}");
    assert_eq!(serde_json::from_str::<Bank>(&json).unwrap(), bank);
}

#[test]
fn the_same_fact_twice_is_kept_once() {
    let (mut bank, _) = jane();
    let review = source(FactKind::Review, "2026-10-27T10:00:00Z");
    let a = add_fact(
        &mut bank,
        "acme",
        "Used Kafka",
        &[" Kafka ".into()],
        review.clone(),
    )
    .unwrap();
    let b = add_fact(
        &mut bank,
        "acme",
        "used  kafka",
        &["Docker".into(), "kafka".into()],
        review,
    )
    .unwrap();
    assert_eq!((a.as_str(), b.as_str()), ("acme~1", "acme~1"));
    let facts = &acme(&bank).facts;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].skills, ["Kafka", "Docker"]);
}

#[test]
fn edits_keep_ids_and_sources_and_undo_brings_a_fact_back_as_it_was() {
    let (mut bank, _) = jane();
    let review = source(FactKind::Review, "2026-10-27T10:00:00Z");
    let typed = source(FactKind::Edit, "2026-10-28T09:00:00Z");
    add_fact(
        &mut bank,
        "acme",
        "Used Kafka",
        &["Kafka".into()],
        review.clone(),
    )
    .unwrap();
    add_fact(
        &mut bank,
        "acme",
        "Ran the on-call rotation",
        &[],
        review.clone(),
    )
    .unwrap();
    let before: Vec<Fact> = acme(&bank).facts.clone();

    // Removing the first and adding one: the kept fact keeps its id; the new one counts on past every id held.
    let drafts = vec![draft(&before[1]), new_draft("  Mentored two interns ")];
    set_facts(&mut bank, "acme", drafts, typed.clone()).unwrap();
    let facts = &acme(&bank).facts;
    let ids: Vec<&str> = facts.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, ["acme~2", "acme~3"]);
    assert_eq!(facts[0].source, review);
    assert_eq!(
        (facts[1].text.as_str(), &facts[1].source),
        ("Mentored two interns", &typed)
    );

    // Undo puts the removed fact back where it was, with its id and where it came from.
    let mut drafts: Vec<FactDraft> = facts.iter().map(draft).collect();
    drafts.insert(0, draft(&before[0]));
    set_facts(&mut bank, "acme", drafts, typed).unwrap();
    assert_eq!(acme(&bank).facts[0], before[0]);
}

#[test]
fn a_fact_needs_text_and_its_own_id() {
    let (mut bank, _) = jane();
    let typed = source(FactKind::Edit, "2026-10-28T09:00:00Z");
    add_fact(&mut bank, "acme", "Used Kafka", &[], typed.clone()).unwrap();
    let before = bank.clone();
    let kept = draft(&acme(&bank).facts[0]);

    assert_eq!(
        set_facts(
            &mut bank,
            "acme",
            vec![kept.clone(), new_draft("   ")],
            typed.clone()
        ),
        Err(FactError::Text)
    );
    assert_eq!(
        set_facts(&mut bank, "acme", vec![kept.clone(), kept], typed.clone()),
        Err(FactError::Duplicate("acme~1".into()))
    );
    assert_eq!(
        add_fact(&mut bank, "nowhere", "Used Kafka", &[], typed),
        Err(FactError::Entry("nowhere".into()))
    );
    assert_eq!(bank, before, "a refused edit changes nothing");
}
