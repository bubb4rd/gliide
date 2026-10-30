use resume_core::master::Master;
use resume_core::variants::{
    VariantError, add_bullet, add_variant, fold_into, make_main, remove_bullet, remove_variant,
    restore_bullet, set_skills, set_wording, show,
};
use resume_core::{Bank, Resume, migrate_v1, resolve};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");

/// Jane's bank with two variants on `acme.1`, and two masters showing every bullet: `swe` its text, `design` its
/// second variant.
fn jane() -> (Bank, Vec<Master>) {
    let m = migrate_v1(&serde_json::from_str::<Resume>(V1).unwrap());
    let mut bank = m.bank;
    let b = &mut bank.sections[0].entries[0].bullets[0];
    b.variants = vec![
        "Batched telemetry into Postgres".into(),
        "Cut ingest latency".into(),
    ];
    let swe = m.master;
    let mut design = swe.clone();
    design.id = "design".into();
    design.title = "Design".into();
    design.sections[0].entries[0].bullets[0].variant = Some(1);
    (bank, vec![swe, design])
}

/// What each master reads, to compare before and after an edit.
fn pages(bank: &Bank, masters: &[Master]) -> Vec<Resume> {
    masters.iter().map(|m| resolve(bank, m).unwrap()).collect()
}

/// Every bullet each master shows, as text.
fn texts(bank: &Bank, masters: &[Master]) -> Vec<Vec<String>> {
    pages(bank, masters)
        .iter()
        .map(|r| {
            r.sections
                .iter()
                .flat_map(|s| &s.entries)
                .flat_map(|e| &e.bullets)
                .map(|b| b.text.clone())
                .collect()
        })
        .collect()
}

fn acme1(bank: &Bank) -> (&str, Vec<&str>) {
    let b = &bank.sections[0].entries[0].bullets[0];
    (
        b.text.as_str(),
        b.variants.iter().map(String::as_str).collect(),
    )
}

#[test]
fn removing_a_variant_renumbers_what_masters_show() {
    let (mut bank, mut masters) = jane();
    let before = pages(&bank, &masters);
    remove_variant(&mut bank, &mut masters, "acme.1", 0).unwrap();
    assert_eq!(acme1(&bank).1, ["Cut ingest latency"]);
    assert_eq!(
        pages(&bank, &masters),
        before,
        "design still reads its variant, now 0"
    );
    assert_eq!(
        masters[1].sections[0].entries[0].bullets[0].variant,
        Some(0)
    );

    // The variant a master shows goes: it falls back to the bullet's text.
    remove_variant(&mut bank, &mut masters, "acme.1", 0).unwrap();
    assert_eq!(masters[1].sections[0].entries[0].bullets[0].variant, None);
    assert_eq!(
        remove_variant(&mut bank, &mut masters, "acme.1", 0),
        Err(VariantError::Variant(0))
    );
}

#[test]
fn making_a_variant_the_text_keeps_every_master_reading_the_same() {
    let (mut bank, mut masters) = jane();
    let before = pages(&bank, &masters);
    let text = acme1(&bank).0.to_owned();
    make_main(&mut bank, &mut masters, "acme.1", 1).unwrap();
    assert_eq!(
        acme1(&bank),
        (
            "Cut ingest latency",
            vec!["Batched telemetry into Postgres", &text]
        )
    );
    assert_eq!(pages(&bank, &masters), before);
    assert_eq!(
        masters[0].sections[0].entries[0].bullets[0].variant,
        Some(1)
    );
    assert_eq!(masters[1].sections[0].entries[0].bullets[0].variant, None);
}

#[test]
fn adding_and_editing_wordings() {
    let (mut bank, mut masters) = jane();
    assert_eq!(
        add_variant(&mut bank, "acme.1", "  Shipped a Rust service  "),
        Ok(2)
    );
    assert_eq!(acme1(&bank).1[2], "Shipped a Rust service");
    assert_eq!(
        add_variant(&mut bank, "acme.1", "Cut  ingest latency "),
        Err(VariantError::Duplicate),
        "already a wording, whitespace aside"
    );
    assert_eq!(
        add_variant(&mut bank, "acme.1", " "),
        Err(VariantError::Empty)
    );
    assert_eq!(
        add_variant(&mut bank, "nope", "x"),
        Err(VariantError::Bullet("nope".into()))
    );

    // Editing a wording changes it in every master that shows it.
    set_wording(&mut bank, "acme.1", Some(1), "Cut ingest latency by 40%").unwrap();
    let design = resolve(&bank, &masters[1]).unwrap();
    assert_eq!(
        design.sections[0].entries[0].bullets[0].text,
        "Cut ingest latency by 40%"
    );
    assert_eq!(
        set_wording(&mut bank, "acme.1", None, "Shipped a Rust service"),
        Err(VariantError::Duplicate)
    );
    // The same words for the same wording are fine.
    set_wording(&mut bank, "acme.1", Some(2), "Shipped a Rust service").unwrap();

    // Choosing what a master shows.
    assert_eq!(show(&bank, &mut masters[0], "acme.1", Some(2)), Ok(true));
    assert_eq!(
        resolve(&bank, &masters[0]).unwrap().sections[0].entries[0].bullets[0].text,
        "Shipped a Rust service"
    );
    assert_eq!(
        show(&bank, &mut masters[0], "acme.1", Some(9)),
        Err(VariantError::Variant(9))
    );
}

#[test]
fn folding_a_bullet_into_another_of_its_entry() {
    let (mut bank, mut masters) = jane();
    let acme2 = bank.sections[0].entries[0].bullets[1].text.clone();
    bank.sections[0].entries[0].bullets[1].skills = vec!["CI".into()];
    // `swe` shows both bullets: folding would lose one of its lines.
    assert_eq!(
        fold_into(&mut bank, &mut masters, "acme.2", "acme.1"),
        Err(VariantError::BothShown("Software Engineering".into()))
    );

    // Take acme.1 out of both masters; they show acme.2's text.
    for m in &mut masters {
        m.sections[0].entries[0]
            .bullets
            .retain(|b| b.id != "acme.1");
    }
    let before = texts(&bank, &masters);
    fold_into(&mut bank, &mut masters, "acme.2", "acme.1").unwrap();
    let entry = &bank.sections[0].entries[0];
    assert!(entry.bullets.iter().all(|b| b.id != "acme.2"));
    assert_eq!(acme1(&bank).1.last(), Some(&acme2.as_str()));
    assert_eq!(entry.bullets[0].skills, ["CI"]);
    assert_eq!(
        texts(&bank, &masters),
        before,
        "the masters read acme.2's words, now a variant of acme.1"
    );

    assert_eq!(
        fold_into(&mut bank, &mut masters, "acme.3", "library.1"),
        Err(VariantError::OtherEntry)
    );
}

#[test]
fn folding_reuses_a_wording_the_target_already_has() {
    let (mut bank, mut masters) = jane();
    bank.sections[0].entries[0].bullets[2].text = "Cut ingest latency".into();
    for m in &mut masters {
        m.sections[0].entries[0]
            .bullets
            .retain(|b| b.id != "acme.1");
    }
    fold_into(&mut bank, &mut masters, "acme.3", "acme.1").unwrap();
    assert_eq!(
        acme1(&bank).1.len(),
        2,
        "no second copy of “Cut ingest latency”"
    );
    assert_eq!(
        masters[0].sections[0].entries[0]
            .bullets
            .last()
            .map(|b| (b.id.as_str(), b.variant)),
        Some(("acme.1", Some(1)))
    );
}

#[test]
fn a_new_bullet_takes_the_number_after_the_highest_in_no_master() {
    let (mut bank, masters) = jane();
    let before = pages(&bank, &masters);
    let id = add_bullet(&mut bank, "acme", "  Ran the on-call rotation ").unwrap();
    assert_eq!(id, "acme.4");
    let acme = &bank.sections[0].entries[0];
    assert_eq!(acme.bullets.last().unwrap().text, "Ran the on-call rotation");
    assert_eq!(pages(&bank, &masters), before, "no master shows it");
    // A bullet removed from the middle keeps its id unused: a story may still name it, and Undo puts it back under it.
    bank.sections[0].entries[0].bullets.retain(|b| b.id != "acme.2");
    assert_eq!(
        add_bullet(&mut bank, "acme", "Mentored 2 interns").unwrap(),
        "acme.5"
    );
    assert_eq!(
        add_bullet(&mut bank, "acme", "Ran the on-call  rotation"),
        Err(VariantError::Duplicate)
    );
    assert_eq!(
        add_bullet(&mut bank, "acme", "Batched telemetry into Postgres"),
        Err(VariantError::Duplicate),
        "a variant's words count"
    );
    assert_eq!(add_bullet(&mut bank, "acme", " "), Err(VariantError::Empty));
    assert_eq!(
        add_bullet(&mut bank, "nope", "x"),
        Err(VariantError::Entry("nope".into()))
    );
}

#[test]
fn removing_a_bullet_takes_it_out_of_every_master_and_restoring_puts_it_back_as_shown() {
    let (bank, masters) = jane();
    let ids: Vec<String> = bank.sections[0].entries[0]
        .bullets
        .iter()
        .map(|b| b.id.clone())
        .collect();
    for (at, id) in ids.iter().enumerate() {
        let (mut b, mut ms) = (bank.clone(), masters.clone());
        let gone = b.sections[0].entries[0].bullets[at].clone();
        let shown: Vec<(String, Option<usize>)> = ms
            .iter()
            .flat_map(|m| {
                m.sections[0].entries[0]
                    .bullets
                    .iter()
                    .filter(|x| x.id == *id)
                    .map(|x| (m.id.clone(), x.variant))
            })
            .collect();
        remove_bullet(&mut b, &mut ms, id).unwrap();
        assert!(b.sections[0].entries[0].bullets.iter().all(|x| x.id != *id));
        for page in texts(&b, &ms) {
            assert!(!page.contains(&gone.text), "{id} left every master");
            assert_eq!(
                page.len(),
                texts(&bank, &masters)[0].len() - 1,
                "the other lines stay"
            );
        }

        restore_bullet(&mut b, &mut ms, "acme", at, gone, &shown).unwrap();
        assert_eq!(
            (&b, &ms),
            (&bank, &masters),
            "{id} is back where it was, as each master showed it"
        );
    }
    assert_eq!(
        remove_bullet(&mut bank.clone(), &mut masters.clone(), "acme.9"),
        Err(VariantError::Bullet("acme.9".into()))
    );
}

#[test]
fn a_restored_bullet_needs_its_id_free_and_skips_a_master_that_left_its_entry() {
    let (mut bank, mut masters) = jane();
    let acme1 = bank.sections[0].entries[0].bullets[0].clone();
    let shown = [("design".to_owned(), Some(1))];
    assert_eq!(
        restore_bullet(&mut bank, &mut masters, "acme", 0, acme1.clone(), &shown),
        Err(VariantError::Taken("acme.1".into()))
    );

    remove_bullet(&mut bank, &mut masters, "acme.1").unwrap();
    masters[1].sections[0].entries.retain(|e| e.id != "acme");
    let design = masters[1].clone();
    restore_bullet(&mut bank, &mut masters, "acme", 0, acme1, &shown).unwrap();
    assert_eq!(bank.sections[0].entries[0].bullets[0].id, "acme.1");
    assert_eq!(
        masters[1], design,
        "a master that no longer shows the entry stays as it is"
    );
}

#[test]
fn skills_are_set_whole_trimmed_and_once() {
    let (mut bank, _) = jane();
    let skills = [" GitHub  Actions ", "", "github actions", "Integration testing"].map(String::from);
    set_skills(&mut bank, "acme.2", &skills).unwrap();
    assert_eq!(
        bank.sections[0].entries[0].bullets[1].skills,
        ["GitHub Actions", "Integration testing"]
    );
    set_skills(&mut bank, "acme.2", &[]).unwrap();
    assert!(bank.sections[0].entries[0].bullets[1].skills.is_empty());
    assert_eq!(
        set_skills(&mut bank, "acme.9", &[]),
        Err(VariantError::Bullet("acme.9".into()))
    );
}
