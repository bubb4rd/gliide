//! Facts: what you've said about an entry that its lines don't print.

use resume_core::bank::{Fact, FactKind, FactSource};
use resume_core::{Bank, Resume, migrate_v1, resolve};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");

fn jane() -> (Bank, resume_core::Master) {
    let m = migrate_v1(&serde_json::from_str::<Resume>(V1).unwrap());
    (m.bank, m.master)
}

#[test]
fn facts_never_reach_the_page() {
    let (mut bank, master) = jane();
    let acme = bank
        .sections
        .iter_mut()
        .flat_map(|s| &mut s.entries)
        .find(|e| e.id == "acme")
        .unwrap();
    acme.facts.push(Fact {
        id: "acme~1".into(),
        text: "The order API handled about 2,000 orders a day".into(),
        skills: vec!["Kafka".into()],
        source: FactSource {
            kind: FactKind::Edit,
            at: "2026-10-27T10:00:00Z".into(),
            posting: None,
        },
    });
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
