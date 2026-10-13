use resume_core::bank::SectionKind;
use resume_core::master::{MasterBullet, MasterGroup};
use resume_core::{Bank, Master, ResolveError, Resume, migrate_v1, resolve};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");

fn v1() -> Resume {
    serde_json::from_str(V1).expect("fixture parses as a v1 resume")
}

#[test]
fn migrate_then_resolve_round_trips() {
    let v1 = v1();
    let m = migrate_v1(&v1);
    assert_eq!(m.master.id, "swe");
    assert_eq!(resolve(&m.bank, &m.master).unwrap(), v1);
    // Also through JSON, as `ez migrate` writes them and `ez render` reads them.
    let bank: Bank = serde_json::from_str(&serde_json::to_string(&m.bank).unwrap()).unwrap();
    let master: Master = serde_json::from_str(&serde_json::to_string(&m.master).unwrap()).unwrap();
    assert_eq!(resolve(&bank, &master).unwrap(), v1);
    // And the resolved JSON equals the fixture's, field for field.
    let fixture: serde_json::Value = serde_json::from_str(V1).unwrap();
    assert_eq!(
        serde_json::to_value(resolve(&bank, &master).unwrap()).unwrap(),
        fixture
    );
}

#[test]
fn migrate_reads_dates_and_kinds() {
    let m = migrate_v1(&v1());
    let entry = |id: &str| {
        m.bank
            .sections
            .iter()
            .flat_map(|s| &s.entries)
            .find(|e| e.id == id)
            .unwrap()
    };
    assert_eq!(
        (entry("acme").start.as_deref(), entry("acme").end.as_deref()),
        (Some("2025-06"), Some("2025-09"))
    );
    assert_eq!(entry("tidepool").end.as_deref(), Some("present"));
    assert_eq!(
        (
            entry("library").start.as_deref(),
            entry("library").end.as_deref()
        ),
        (None, None)
    );
    assert_eq!(m.warnings.len(), 1);
    assert!(m.warnings[0].contains("library"), "{:?}", m.warnings);
    let kinds: Vec<_> = m.bank.sections.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [
            SectionKind::Experience,
            SectionKind::Projects,
            SectionKind::Education,
            SectionKind::Skills
        ]
    );
}

#[test]
fn resolve_picks_variants_items_and_titles() {
    let mut m = migrate_v1(&v1());
    m.bank.sections[0].entries[0].bullets[1].variants =
        vec!["Added CI integration tests for the order API".into()];
    let work = &mut m.master.sections[0];
    work.title = Some("EXPERIENCE".into());
    work.entries[0].bullets = vec![MasterBullet {
        id: "acme.2".into(),
        variant: Some(0),
    }];
    m.master.sections[3].groups = vec![MasterGroup {
        id: "languages".into(),
        items: vec!["SQL".into(), "Rust".into()],
    }];
    let r = resolve(&m.bank, &m.master).unwrap();
    assert_eq!(r.sections[0].title, "EXPERIENCE");
    assert_eq!(r.sections[0].entries[0].bullets.len(), 1);
    assert_eq!(
        r.sections[0].entries[0].bullets[0].text,
        "Added CI integration tests for the order API"
    );
    assert_eq!(r.sections[3].groups[0].items, ["SQL", "Rust"]);
}

#[test]
fn resolve_errors_name_the_unknown_id() {
    let base = migrate_v1(&v1());

    let mut m = base.master.clone();
    m.sections[0].id = "nope".into();
    let err = resolve(&base.bank, &m).unwrap_err();
    assert_eq!(err, ResolveError::Section { id: "nope".into() });
    assert!(err.to_string().contains("`nope`"));

    let mut m = base.master.clone();
    m.sections[0].entries[0].id = "ghost".into();
    assert!(
        resolve(&base.bank, &m)
            .unwrap_err()
            .to_string()
            .contains("`ghost`")
    );

    let mut m = base.master.clone();
    m.sections[0].entries[0].bullets[0].id = "acme.9".into();
    assert_eq!(
        resolve(&base.bank, &m).unwrap_err(),
        ResolveError::Bullet {
            entry: "acme".into(),
            id: "acme.9".into()
        }
    );

    let mut m = base.master.clone();
    m.sections[0].entries[0].bullets[0].variant = Some(2);
    assert!(matches!(
        resolve(&base.bank, &m).unwrap_err(),
        ResolveError::Variant {
            variant: 2,
            count: 0,
            ..
        }
    ));

    let mut m = base.master.clone();
    m.sections[3].groups[0].id = "spoken".into();
    assert!(
        resolve(&base.bank, &m)
            .unwrap_err()
            .to_string()
            .contains("`spoken`")
    );

    let mut m = base.master.clone();
    m.sections[3].groups[0].items.push("COBOL".into());
    assert_eq!(
        resolve(&base.bank, &m).unwrap_err(),
        ResolveError::Item {
            group: "languages".into(),
            item: "COBOL".into()
        }
    );
}

#[test]
fn schemas_generate() {
    let bank = serde_json::to_value(resume_core::bank_schema()).unwrap();
    assert_eq!(bank["title"], "Bank");
    let master = serde_json::to_value(resume_core::master_schema()).unwrap();
    assert!(master["properties"]["pageLimit"].is_object());
}
