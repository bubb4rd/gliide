//! The change engine on synthetic data: the Jane Doe bank, a master without her library job, two bank-only projects, and
//! hand-made suggestions through `changes_of`, `draft_changes` and `apply`.

use resume_core::apply::{ApplyError, apply};
use resume_core::bank::{BankBullet, BankEntry, Fact, FactKind, FactSource};
use resume_core::candidates::bank_candidates;
use resume_core::change;
use resume_core::change::{Change, Draft, Mode, Op, Origin};
use resume_core::changes::{changes_of, draft_changes};
use resume_core::resume::{Bullet, SectionKind};
use resume_core::suggestions::{
    BankEdit, BankOp, BulletEdit, BulletOp, Evidence, Have, Keyword, KeywordOp, Order, SkillEdit,
    SkillOp, Suggestions, Verdict,
};
use resume_core::{Bank, Master, Requirements, Resume, migrate_v1};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");
const REQ: &str = include_str!("../../../fixtures/score/jane-doe-swe.requirements.json");

const ACME_1: &str = "Built a Rust service that batches widget telemetry into Postgres, cutting ingest latency from \
                      900 ms to 120 ms across 40 production lines";
const ACME_2: &str = "Wrote integration tests in GitHub Actions for the order API, catching 14 regressions before release";
const ACME_3: &str =
    "Paired with the design team to ship a React dashboard that plant managers now use daily";
const ACME_3_V0: &str =
    "Shipped a React dashboard with the design team that plant managers now use daily";

fn bank_entry(id: &str, name: &str, subtitle: &str, bullets: &[&str]) -> BankEntry {
    BankEntry {
        id: id.into(),
        name: name.into(),
        url: String::new(),
        place: String::new(),
        dates: "2024".into(),
        start: Some("2024-01".into()),
        end: Some("2024-12".into()),
        subtitle: subtitle.into(),
        tags: vec![],
        notes: vec![],
        bullets: bullets
            .iter()
            .enumerate()
            .map(|(i, t)| BankBullet {
                id: format!("{id}.{}", i + 1),
                text: (*t).into(),
                variants: vec![],
                skills: vec![],
            })
            .collect(),
        facts: vec![],
    }
}

/// Jane's bank with a variant on `acme.3` and two projects no master shows (`kiln`, `pixel`); the `swe` master without
/// her library job.
fn setup() -> (Bank, Master) {
    let m = migrate_v1(&serde_json::from_str(V1).unwrap());
    let (mut bank, mut master) = (m.bank, m.master);
    let work = bank.sections.iter_mut().find(|s| s.id == "work").unwrap();
    let acme = work.entries.iter_mut().find(|e| e.id == "acme").unwrap();
    acme.bullets[2].variants = vec![ACME_3_V0.into()];
    let projects = bank
        .sections
        .iter_mut()
        .find(|s| s.id == "projects")
        .unwrap();
    projects.entries.push(bank_entry(
        "kiln",
        "Kiln",
        "Creator",
        &[
            "Wrote a Rust job runner packaged with Docker that schedules 2,000 builds a day",
            "Added retries and backoff to flaky network calls",
        ],
    ));
    projects.entries.push(bank_entry(
        "pixel",
        "Pixel Studio",
        "Freelance Designer",
        &["Designed posters and brand kits for local shops"],
    ));
    let work = master.sections.iter_mut().find(|s| s.id == "work").unwrap();
    work.entries.retain(|e| e.id != "library");
    (bank, master)
}

fn req() -> Requirements {
    serde_json::from_str(REQ).unwrap()
}

fn evidence(skill: &str, have: Have) -> Evidence {
    Evidence {
        skill: skill.into(),
        have,
        ids: vec![],
    }
}

/// Suggestions with Jane's evidence and nothing to change.
fn base() -> Suggestions {
    Suggestions {
        verdict: Verdict::Tailor,
        headline: "Strong Rust backend fit".into(),
        evidence: vec![
            evidence("Rust", Have::Yes),
            evidence("Amazon Web Services", Have::Add),
            evidence("Integration test", Have::Yes),
            evidence("React", Have::Yes),
            evidence("Vue", Have::No),
            evidence("MongoDB", Have::No),
            evidence("Docker", Have::Yes),
            evidence("Telemetry", Have::Yes),
            evidence("Mentoring", Have::Add),
        ],
        keywords: vec![],
        bullets: vec![],
        skills: vec![],
        bank: vec![],
        order: vec![],
        summary: None,
        gaps: vec![],
    }
}

fn rewrite(id: &str, before: &str, after: &str, sources: &[&str]) -> BulletEdit {
    BulletEdit {
        op: BulletOp::Rewrite,
        id: id.into(),
        before: before.into(),
        after: after.into(),
        hits: vec![],
        sources: sources.iter().map(|&s| s.into()).collect(),
        variant: None,
        priority: 1,
        why: "states the posting's term".into(),
    }
}

fn add_bullet(entry: &str, after: &str, sources: &[&str]) -> BulletEdit {
    BulletEdit {
        op: BulletOp::Add,
        before: String::new(),
        ..rewrite(entry, "", after, sources)
    }
}

fn keyword(op: KeywordOp, line: &str, from: &str, term: &str, skill: &str) -> Keyword {
    Keyword {
        op,
        line: line.into(),
        term: term.into(),
        from: from.into(),
        skill: skill.into(),
        priority: 2,
        why: String::new(),
    }
}

fn skill(op: SkillOp, group: &str, item: &str, to: &str, skill: &str) -> SkillEdit {
    SkillEdit {
        op,
        group: group.into(),
        item: item.into(),
        to: to.into(),
        skill: skill.into(),
        priority: 2,
        why: String::new(),
    }
}

/// One suggestion of every kind, all grounded.
fn every_op() -> Suggestions {
    let mut s = base();
    s.bullets = vec![
        BulletEdit {
            hits: vec!["Integration test".into()],
            ..rewrite(
                "acme.2",
                ACME_2,
                "Wrote integration tests in GitHub Actions for the order API, catching 14 regressions before each \
                 release",
                &["acme.2"],
            )
        },
        BulletEdit {
            variant: Some(0),
            ..rewrite("acme.3", ACME_3, ACME_3_V0, &["acme.3"])
        },
        BulletEdit {
            op: BulletOp::Cut,
            after: String::new(),
            ..rewrite(
                "state-u.1",
                "Teaching assistant for Data Structures, holding weekly office hours for 80 students",
                "",
                &[],
            )
        },
        add_bullet(
            "acme",
            "Monitored widget telemetry from 40 production lines in Postgres",
            &["acme.1"],
        ),
    ];
    s.keywords = vec![
        keyword(KeywordOp::Edit, "acme.1", "Postgres", "PostgreSQL", ""),
        keyword(KeywordOp::Add, "state-u#1", "", "Mentoring", "Mentoring"),
        keyword(KeywordOp::Drop, "state-u#1", "", "Operating Systems", ""),
        // On a bullet that is rewritten: the rewrite carries it.
        keyword(KeywordOp::Edit, "acme.2", "tests", "testing", ""),
    ];
    s.skills = vec![
        skill(
            SkillOp::Add,
            "tools",
            "Amazon Web Services",
            "",
            "Amazon Web Services",
        ),
        // Evidence says no: never added.
        skill(SkillOp::Add, "languages", "Vue", "", "Vue"),
        skill(SkillOp::Remove, "tools", "SQLite", "", ""),
        skill(SkillOp::Rename, "tools", "React", "React.js", "React"),
        skill(SkillOp::Move, "languages", "SQL", "tools", ""),
    ];
    s.bank = vec![
        BankEdit {
            op: BankOp::Include,
            entry: "kiln".into(),
            bullets: vec!["kiln.1".into()],
            replaces: String::new(),
            skills: vec!["Rust".into(), "Docker".into()],
            why: "Rust and Docker".into(),
        },
        BankEdit {
            op: BankOp::Swap,
            entry: "pixel".into(),
            bullets: vec![],
            replaces: "tidepool".into(),
            skills: vec![],
            why: "design range".into(),
        },
    ];
    s.order = vec![Order {
        section: "projects".into(),
        entries: vec!["kiln".into(), "pixel".into()],
    }];
    s.summary =
        Some("Software engineering intern who builds Rust services and React dashboards".into());
    s
}

fn op_names(cs: &[Change]) -> Vec<&'static str> {
    cs.iter().map(|c| c.op.target().0).collect()
}

fn find<'a>(cs: &'a [Change], name: &str, target: &str) -> &'a Change {
    cs.iter()
        .find(|c| c.op.target() == (name, target.to_owned()))
        .unwrap_or_else(|| panic!("no {name} {target} in {:#?}", op_names(cs)))
}

fn bullet<'a>(r: &'a Resume, id: &str) -> &'a Bullet {
    r.sections
        .iter()
        .flat_map(|s| &s.entries)
        .flat_map(|e| &e.bullets)
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("no bullet {id}"))
}

fn on(r: &Resume, entry: &str) -> bool {
    r.sections
        .iter()
        .flat_map(|s| &s.entries)
        .any(|e| e.id == entry)
}

fn all(cs: &[Change]) -> Vec<&Change> {
    cs.iter().collect()
}

#[test]
fn every_op_through_changes_of_and_apply() {
    let (bank, master) = setup();
    let cs = changes_of(&every_op(), &bank, &master, &req(), Mode::BankAware);
    for c in &cs {
        assert!(c.flags.is_empty(), "{:?}: {:?}", c.op, c.flags);
        assert_eq!(c.origin, Origin::Suggested);
    }
    assert_eq!(
        op_names(&cs),
        [
            "rewrite",
            "use_variant",
            "cut",
            "add",
            "swap",
            "note_add",
            "swap",
            "skill_add",
            "skill_remove",
            "skill_rename",
            "skill_move",
            "include",
            "swap_entry",
            "reorder",
            "summary"
        ]
    );
    // `before` comes from the master; sections name where each change lands.
    let rw = find(&cs, "rewrite", "acme.2");
    assert_eq!((rw.before.as_str(), rw.section.as_str()), (ACME_2, "work"));
    assert_eq!(
        find(&cs, "use_variant", "acme.3").op,
        Op::UseVariant {
            bullet: "acme.3".into(),
            variant: 0
        }
    );
    assert_eq!(
        find(&cs, "note_add", "state-u#1/mentoring").section,
        "education"
    );
    assert_eq!(
        find(&cs, "skill_add", "tools/amazon web services").hits,
        ["Amazon Web Services"]
    );
    assert_eq!(find(&cs, "include", "kiln").section, "bank");
    assert_eq!(find(&cs, "swap_entry", "tidepool>pixel").before, "Tidepool");
    assert_eq!(find(&cs, "reorder", "projects").before, "tidepool");
    let summary = find(&cs, "summary", "");
    assert_eq!(summary.hits, ["Rust", "React"]);

    // Reversed input: apply orders the ops itself.
    let mut refs = all(&cs);
    refs.reverse();
    let r = apply(&bank, &master, &refs).unwrap();

    let b2 = bullet(&r, "acme.2");
    assert!(b2.text.ends_with("before each release"));
    assert_eq!(b2.orig.as_deref(), Some(ACME_2));
    assert!(b2.changed && !b2.new);
    let b3 = bullet(&r, "acme.3");
    assert_eq!(
        (b3.text.as_str(), b3.orig.as_deref()),
        (ACME_3_V0, Some(ACME_3))
    );
    let b1 = bullet(&r, "acme.1");
    assert!(b1.text.contains("into PostgreSQL, cutting"));
    assert_eq!(b1.orig.as_deref(), Some(ACME_1));
    let added = find(&cs, "add", "acme");
    let nb = bullet(&r, &added.key);
    assert_eq!(
        nb.text,
        "Monitored widget telemetry from 40 production lines in Postgres"
    );
    assert!(nb.new && nb.changed && nb.orig.is_none());
    let acme = r
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == "acme")
        .unwrap();
    assert_eq!(acme.bullets.last().unwrap().id, added.key);

    let state = r
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == "state-u")
        .unwrap();
    assert!(state.bullets.is_empty());
    assert_eq!(
        state.notes[0].text,
        "Data Structures, Databases, Computer Networks, Mentoring"
    );
    assert!(state.notes[0].changed && !state.notes[1].changed);

    let groups: Vec<_> = r.sections.iter().flat_map(|s| &s.groups).collect();
    let (langs, tools) = (groups[0], groups[1]);
    assert_eq!(langs.items, ["Rust", "TypeScript", "Python", "Java"]);
    assert_eq!(
        langs.orig.as_deref().unwrap(),
        ["Rust", "TypeScript", "Python", "SQL", "Java"]
    );
    assert!(langs.added.is_empty());
    assert_eq!(
        tools.items,
        [
            "Git",
            "GitHub Actions",
            "Docker",
            "PostgreSQL",
            "React.js",
            "Linux",
            "AWS",
            "SQL",
            "Amazon Web Services"
        ]
    );
    assert_eq!(tools.added, ["React.js", "SQL", "Amazon Web Services"]);

    let projects = r.sections.iter().find(|s| s.id == "projects").unwrap();
    let ids: Vec<_> = projects.entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["kiln", "pixel"]);
    assert_eq!(projects.entries[0].bullets.len(), 1);
    assert!(
        projects
            .entries
            .iter()
            .flat_map(|e| &e.bullets)
            .all(|b| b.new && b.changed)
    );
    assert!(!on(&r, "tidepool"));

    let s0 = &r.sections[0];
    assert_eq!(
        (s0.id.as_str(), s0.kind),
        ("summary", Some(SectionKind::Summary))
    );
    assert_eq!(s0.entries[0].notes[0].text, summary.op_text());
    assert!(s0.entries[0].notes[0].changed);

    // Nothing on: the master as resolved.
    assert_eq!(
        apply(&bank, &master, &[]).unwrap(),
        resume_core::resolve(&bank, &master).unwrap()
    );
}

trait OpText {
    fn op_text(&self) -> &str;
}

impl OpText for Change {
    fn op_text(&self) -> &str {
        match &self.op {
            Op::Summary { text }
            | Op::Set { text, .. }
            | Op::Rewrite { text, .. }
            | Op::Add { text, .. } => text,
            _ => "",
        }
    }
}

#[test]
fn master_only_ignores_bank_edits() {
    let (bank, master) = setup();
    let cs = changes_of(&every_op(), &bank, &master, &req(), Mode::MasterOnly);
    let names = op_names(&cs);
    assert!(!names.contains(&"include") && !names.contains(&"swap_entry"));
    // An order naming entries the master doesn't show is flagged in master-only mode.
    assert_eq!(
        find(&cs, "reorder", "projects").flags,
        [
            "“kiln” is not in this section",
            "“pixel” is not in this section"
        ]
    );
}

#[test]
fn keys_survive_a_rerun_with_other_wording() {
    let (bank, master) = setup();
    let first = changes_of(&every_op(), &bank, &master, &req(), Mode::BankAware);
    let mut again = every_op();
    again.bullets[0].after = "Built integration tests in GitHub Actions for the order API".into();
    again.bullets[0].before = "stale copy".into();
    again.bullets[3].after = "Tracked telemetry from 40 production lines".into();
    again.summary = Some("Rust engineer".into());
    let second = changes_of(&again, &bank, &master, &req(), Mode::BankAware);
    let keys = |cs: &[Change]| cs.iter().map(|c| c.key.clone()).collect::<Vec<_>>();
    assert_eq!(keys(&first), keys(&second));
    assert_eq!(
        first,
        changes_of(&every_op(), &bank, &master, &req(), Mode::BankAware)
    );
    // A different line or a different master text is a different change.
    assert_ne!(
        find(&first, "rewrite", "acme.2").key,
        find(&first, "use_variant", "acme.3").key
    );
}

#[test]
fn identical_keys_are_kept_once() {
    let (bank, master) = setup();
    let mut s = base();
    s.skills = vec![
        skill(
            SkillOp::Add,
            "tools",
            "Amazon Web Services",
            "",
            "Amazon Web Services",
        ),
        skill(SkillOp::Add, "tools", "amazon web services", "", ""),
    ];
    let cs = changes_of(&s, &bank, &master, &req(), Mode::MasterOnly);
    assert_eq!(cs.len(), 1);
}

#[test]
fn new_bullets_under_one_entry_get_their_own_keys() {
    let (bank, master) = setup();
    let mut s = base();
    s.bullets = vec![
        add_bullet("acme", "Tuned 40 production lines", &["acme.1"]),
        add_bullet("acme", "Caught 14 regressions", &["acme.2"]),
        add_bullet("acme", "Shipped a React dashboard", &["acme.3"]),
    ];
    let cs = changes_of(&s, &bank, &master, &req(), Mode::MasterOnly);
    let keys: Vec<&str> = cs.iter().map(|c| c.key.as_str()).collect();
    let op = &cs[0].op;
    assert_eq!(
        keys,
        [
            change::key(op, ""),
            change::key(op, "#2"),
            change::key(op, "#3")
        ]
    );
    let r = apply(&bank, &master, &all(&cs)).unwrap();
    assert_eq!(bullet(&r, keys[2]).text, "Shipped a React dashboard");
}

fn flags_of(s: &Suggestions) -> Vec<String> {
    let (bank, master) = setup();
    let cs = changes_of(s, &bank, &master, &req(), Mode::MasterOnly);
    assert_eq!(cs.len(), 1, "{cs:#?}");
    cs[0].flags.clone()
}

fn one_rewrite(after: &str, sources: &[&str]) -> Suggestions {
    let mut s = base();
    s.bullets = vec![rewrite("acme.1", ACME_1, after, sources)];
    s
}

#[test]
fn invented_number_is_flagged() {
    let flags = flags_of(&one_rewrite(
        "Built a Rust service batching telemetry into Postgres, cutting ingest latency 87% across 40 lines",
        &["acme.1"],
    ));
    assert_eq!(flags, ["“87%” is not in the lines it draws on"]);
}

#[test]
fn number_in_a_source_passes() {
    // 14 comes from acme.2, named as a source.
    let s = one_rewrite(
        "Built a Rust service batching telemetry into Postgres, cutting latency to 120 ms; caught 14 regressions",
        &["acme.1", "acme.2"],
    );
    assert!(flags_of(&s).is_empty());
    // Without the source it is invented.
    let s = one_rewrite(
        "Built a Rust service batching telemetry into Postgres, cutting latency to 120 ms; caught 14 regressions",
        &["acme.1"],
    );
    assert_eq!(flags_of(&s), ["“14” is not in the lines it draws on"]);
    // A new bullet checks its sources only.
    let mut s = base();
    s.bullets = vec![add_bullet("acme", "Tuned 40 production lines", &[])];
    assert_eq!(flags_of(&s), ["“40” is not in the lines it draws on"]);
    s.bullets = vec![add_bullet("acme", "Tuned 40 production lines", &["acme.1"])];
    assert!(flags_of(&s).is_empty());
}

#[test]
fn invented_tool_is_flagged() {
    let flags = flags_of(&one_rewrite(
        "Built a Rust service streaming telemetry through Kafka into Postgres on k8s across 40 production lines",
        &["acme.1"],
    ));
    assert_eq!(
        flags,
        ["“Kafka” is not in your bank", "“k8s” is not in your bank"]
    );
    // A requirement skill the evidence says you lack is not grounded either.
    let flags = flags_of(&one_rewrite(
        "Built a Rust service batching telemetry into MongoDB across 40 production lines",
        &["acme.1"],
    ));
    assert_eq!(flags, ["“MongoDB” is not in your bank"]);
}

#[test]
fn requirement_skill_with_evidence_passes() {
    // "Vue" is nowhere in the bank: flagged while the evidence says `no`, grounded once it says `add`.
    let mut s = base();
    s.bullets = vec![rewrite(
        "acme.3",
        ACME_3,
        "Paired with the design team to ship a React and Vue dashboard that plant managers now use daily",
        &["acme.3"],
    )];
    assert_eq!(flags_of(&s), ["“Vue” is not in your bank"]);
    s.evidence
        .iter_mut()
        .find(|e| e.skill == "Vue")
        .unwrap()
        .have = Have::Add;
    assert!(flags_of(&s).is_empty());
    // Every word of a backed multi-word skill passes; "AWS" in the bank grounds its alias too.
    let s = one_rewrite(
        "Built a Rust service on Amazon Web Services batching telemetry into Postgres across 40 production lines",
        &["acme.1"],
    );
    assert!(flags_of(&s).is_empty());
    let mut s = base();
    s.skills = vec![skill(SkillOp::Add, "tools", "Kafka", "", "")];
    assert_eq!(flags_of(&s), ["“Kafka” is not in your bank"]);
}

#[test]
fn stale_before_and_unknown_ids_are_flagged() {
    let mut s = one_rewrite(
        "Built a Rust service that batches telemetry into Postgres across 40 production lines",
        &["acme.1", "acme.9"],
    );
    s.bullets[0].before = "Built a Go service that batches telemetry".into();
    assert_eq!(
        flags_of(&s),
        [
            "written for different text than this line has now",
            "cites “acme.9”, which is not in your bank"
        ]
    );
    // Whitespace aside, the same text is not stale.
    s.bullets[0].before = ACME_1.replace(' ', "  ");
    s.bullets[0].sources = vec!["acme.1".into()];
    assert!(flags_of(&s).is_empty());
    // An edit to a line the master doesn't show is dropped.
    let (bank, master) = setup();
    let mut s = base();
    s.bullets = vec![rewrite(
        "library.1",
        "",
        "Imaged 60 laptops",
        &["library.1"],
    )];
    assert!(changes_of(&s, &bank, &master, &req(), Mode::MasterOnly).is_empty());
}

#[test]
fn sources_under_another_entry_are_flagged() {
    let after =
        "Built a Rust service that batches telemetry into Postgres across 40 production lines";
    // A work bullet citing a coursework note.
    let s = one_rewrite(after, &["acme.1", "state-u#1"]);
    assert_eq!(
        flags_of(&s),
        [
            "draws on “Relevant Coursework: Data Structures, Operating Systems…” from State University, not this \
             entry"
        ]
    );
    // A new bullet citing another entry's bullet, or the entry itself.
    let mut s = base();
    s.bullets = vec![add_bullet(
        "acme",
        "Designed an offline-first app",
        &["tidepool.1", "tidepool"],
    )];
    assert_eq!(
        flags_of(&s),
        [
            "draws on “Designed an offline-first notes app in…” from Tidepool, not this entry",
            "draws on Tidepool, not this entry"
        ]
    );
}

#[test]
fn sources_in_the_same_entry_pass() {
    let s = one_rewrite(
        "Built a Rust service batching telemetry into Postgres, cutting latency to 120 ms; caught 14 regressions",
        &["acme.1", "acme.2", "acme"],
    );
    assert!(flags_of(&s).is_empty());
    let mut s = base();
    s.bullets = vec![add_bullet(
        "acme",
        "Tuned 40 production lines",
        &["acme.1", "acme"],
    )];
    assert!(flags_of(&s).is_empty());
}

#[test]
fn facts_back_lines_of_their_own_entry_only() {
    let (mut bank, master) = setup();
    let fact = |id: &str, text: &str, skills: &[&str]| Fact {
        id: id.into(),
        text: text.into(),
        skills: skills.iter().map(|&s| s.into()).collect(),
        source: FactSource {
            kind: FactKind::Review,
            at: "2026-10-27T10:00:00Z".into(),
            posting: None,
        },
    };
    for e in bank.sections.iter_mut().flat_map(|s| &mut s.entries) {
        match e.id.as_str() {
            "acme" => {
                e.facts = vec![fact(
                    "acme~1",
                    "The service took 2,000 events a second",
                    &["Kafka"],
                )]
            }
            "tidepool" => e.facts = vec![fact("tidepool~1", "300 people used it offline", &[])],
            _ => {}
        }
    }
    let flags = |after: &str, sources: &[&str]| {
        let mut s = base();
        s.bullets = vec![rewrite("acme.1", ACME_1, after, sources)];
        let cs = changes_of(&s, &bank, &master, &req(), Mode::MasterOnly);
        assert_eq!(cs.len(), 1, "{cs:#?}");
        cs[0].flags.clone()
    };
    let after = "Built a Rust service taking 2,000 events a second through Kafka into Postgres across 40 lines";
    // The entry's own fact backs its number and its skill.
    assert!(flags(after, &["acme.1", "acme~1"]).is_empty());
    assert!(
        flags(after, &["acme.1", "acme"]).is_empty(),
        "the entry as a source holds its facts"
    );
    // Uncited, the number has no source.
    assert_eq!(
        flags(after, &["acme.1"]),
        ["“2,000” is not in the lines it draws on"]
    );
    // Another entry's fact is another entry's line.
    assert_eq!(
        flags(
            "Built a Rust service that 300 people used, batching telemetry into Postgres across 40 lines",
            &["acme.1", "tidepool~1"]
        ),
        ["draws on “300 people used it offline” from Tidepool, not this entry"]
    );
}

#[test]
fn skills_line_sources_back_only_evidenced_skills() {
    // "Amazon Web Services" has evidence `add`; the tools line (AWS) backs it.
    let s = one_rewrite(
        "Built a Rust service on Amazon Web Services batching telemetry into Postgres across 40 production lines",
        &["acme.1", "tools"],
    );
    assert!(flags_of(&s).is_empty());
    // A line stating no skill your evidence backs can't lean on a skills line.
    let mut s = base();
    s.bullets = vec![rewrite(
        "tidepool.2",
        "Grew it to 300 weekly users through a public beta and weekly release notes",
        "Grew it to 300 weekly users through a public beta and release notes",
        &["tidepool.2", "languages"],
    )];
    assert_eq!(
        flags_of(&s),
        [
            "draws on your Languages line, which backs only a skill this posting asks for and you have"
        ]
    );
}

#[test]
fn degree_abbreviations_ground_on_the_level() {
    let (mut bank, master) = setup();
    let edu = bank
        .sections
        .iter_mut()
        .find(|s| s.id == "education")
        .unwrap();
    edu.entries[0].subtitle = "Bachelor of Science in Engineering".into();
    let summary = |text: &str| {
        let mut s = base();
        s.summary = Some(text.into());
        let cs = changes_of(&s, &bank, &master, &req(), Mode::MasterOnly);
        assert_eq!(cs.len(), 1);
        cs[0].flags.clone()
    };
    for text in [
        "B.S. graduate who builds Rust services",
        "B.S graduate who builds Rust services",
        "Engineer with a BS who builds Rust services",
    ] {
        assert!(summary(text).is_empty(), "{text}");
    }
    assert_eq!(
        summary("M.S. graduate who builds Rust services"),
        ["“M.S” is not in your bank"]
    );
    assert_eq!(
        summary("Rust engineer with an MBA"),
        ["“MBA” is not in your bank"]
    );
}

#[test]
fn swaps_are_checked() {
    let mut s = base();
    s.keywords = vec![keyword(
        KeywordOp::Edit,
        "acme.3",
        "Paired",
        "Collaboration",
        "",
    )];
    assert_eq!(
        flags_of(&s),
        ["check it reads right (“Paired” → “Collaboration” changes the word's form)"]
    );
    s.keywords = vec![keyword(
        KeywordOp::Edit,
        "acme.3",
        "Paired",
        "Partnered",
        "",
    )];
    assert!(flags_of(&s).is_empty());
    s.keywords = vec![keyword(
        KeywordOp::Edit,
        "acme.3",
        "the art team",
        "the design team",
        "",
    )];
    assert_eq!(flags_of(&s), ["“the art team” is not in this line"]);
}

#[test]
fn draft_wording_becomes_a_set() {
    let (bank, master) = setup();
    let cs = changes_of(&every_op(), &bank, &master, &req(), Mode::BankAware);
    let (rw, var, swap, note, add) = (
        find(&cs, "rewrite", "acme.2").clone(),
        find(&cs, "use_variant", "acme.3").clone(),
        find(&cs, "swap", "acme.1/postgres").clone(),
        find(&cs, "note_add", "state-u#1/mentoring").clone(),
        find(&cs, "add", "acme").clone(),
    );
    let mut draft = Draft::default();
    for (c, text) in [
        (
            &rw,
            "Wrote integration tests for the order API in GitHub Actions",
        ),
        (&var, "Shipped a React dashboard for plant managers"),
        (&swap, "Built a Rust service for widget telemetry"),
        (&note, "Data Structures, Databases"),
        (&add, "Watched telemetry in Postgres"),
    ] {
        draft.wording.insert(c.key.clone(), text.into());
    }
    draft
        .wording
        .insert(find(&cs, "cut", "state-u.1").key.clone(), "ignored".into());
    let mine = Change::new(
        Op::SkillAdd {
            group: "tools".into(),
            item: "Kubernetes".into(),
        },
        Origin::Suggested,
        String::new(),
        vec![],
        String::new(),
        1,
        "skills".into(),
    );
    draft.custom.push(mine.clone());
    let out = draft_changes(&cs, &draft);
    assert_eq!(out.len(), cs.len() + 1);
    let by_key = |k: &str| out.iter().find(|c| c.key == k).unwrap();

    let set = by_key(&rw.key);
    assert_eq!(
        set.op,
        Op::Set {
            line: "acme.2".into(),
            text: "Wrote integration tests for the order API in GitHub Actions".into()
        }
    );
    assert_eq!((set.origin, set.before.as_str()), (Origin::Yours, ACME_2));
    assert_eq!(set.hits, ["Integration test"]);
    assert!(matches!(&by_key(&var.key).op, Op::Set { line, .. } if line == "acme.3"));
    assert!(matches!(&by_key(&swap.key).op, Op::Set { line, .. } if line == "acme.1"));
    assert!(matches!(&by_key(&note.key).op, Op::Set { line, .. } if line == "state-u#1"));
    assert_eq!(
        by_key(&add.key).op,
        Op::Add {
            entry: "acme".into(),
            text: "Watched telemetry in Postgres".into()
        }
    );
    let cut = by_key(&find(&cs, "cut", "state-u.1").key);
    assert_eq!(cut.origin, Origin::Suggested);
    let last = out.last().unwrap();
    assert_eq!(
        (last.key.as_str(), last.origin),
        (mine.key.as_str(), Origin::Yours)
    );

    let r = apply(&bank, &master, &all(&out)).unwrap();
    let b2 = bullet(&r, "acme.2");
    assert_eq!(
        (b2.text.as_str(), b2.orig.as_deref()),
        (
            "Wrote integration tests for the order API in GitHub Actions",
            Some(ACME_2)
        )
    );
    assert_eq!(
        bullet(&r, "acme.1").text,
        "Built a Rust service for widget telemetry"
    );
    let state = r
        .sections
        .iter()
        .flat_map(|s| &s.entries)
        .find(|e| e.id == "state-u")
        .unwrap();
    // Your text for the note line wins over the suggested drop of "Operating Systems" on it.
    assert_eq!(state.notes[0].text, "Data Structures, Databases");
}

#[test]
fn apply_errors_name_the_change() {
    let (bank, master) = setup();
    let change = |op: Op| {
        Change::new(
            op,
            Origin::Yours,
            String::new(),
            vec![],
            String::new(),
            1,
            "work".into(),
        )
    };
    let err = |c: &Change| apply(&bank, &master, &[c]).unwrap_err();

    let c = change(Op::Cut {
        bullet: "acme.9".into(),
    });
    assert_eq!(
        err(&c),
        ApplyError::Unknown {
            key: c.key.clone(),
            what: "bullet",
            id: "acme.9".into()
        }
    );
    let c = change(Op::SkillRemove {
        group: "tools".into(),
        item: "Kafka".into(),
    });
    let e = err(&c);
    assert_eq!(e.key(), Some(c.key.as_str()));
    assert!(matches!(e, ApplyError::Impossible { .. }));
    assert_eq!(
        e.to_string(),
        format!("change {}: “Kafka” isn't on skills line `tools`", c.key)
    );
    let c = change(Op::Swap {
        line: "acme.1".into(),
        from: "Go".into(),
        to: "Rust".into(),
    });
    assert!(matches!(err(&c), ApplyError::Impossible { key, .. } if key == c.key));
    let c = change(Op::UseVariant {
        bullet: "acme.3".into(),
        variant: 4,
    });
    assert!(matches!(err(&c), ApplyError::Impossible { .. }));
    let c = change(Op::Include {
        entry: "acme".into(),
        bullets: vec![],
    });
    assert!(matches!(err(&c), ApplyError::Impossible { .. }));
    let c = change(Op::Include {
        entry: "kiln".into(),
        bullets: vec!["kiln.7".into()],
    });
    assert!(matches!(
        err(&c),
        ApplyError::Unknown { what: "bullet", .. }
    ));
    let c = change(Op::NoteAdd {
        note: "acme#1".into(),
        term: "Rust".into(),
    });
    assert!(matches!(
        err(&c),
        ApplyError::Unknown {
            what: "note line",
            ..
        }
    ));
    let c = change(Op::SkillAdd {
        group: "frameworks".into(),
        item: "Axum".into(),
    });
    assert!(matches!(
        err(&c),
        ApplyError::Unknown {
            what: "skills line",
            ..
        }
    ));
}

#[test]
fn swaps_keep_the_first_word_capitalized() {
    let (bank, master) = setup();
    let swap = |from: &str, to: &str| {
        Change::new(
            Op::Swap {
                line: "acme.3".into(),
                from: from.into(),
                to: to.into(),
            },
            Origin::Suggested,
            ACME_3.into(),
            vec![],
            String::new(),
            2,
            "work".into(),
        )
    };
    let text = |c: &Change| {
        bullet(&apply(&bank, &master, &[c]).unwrap(), "acme.3")
            .text
            .clone()
    };
    assert!(text(&swap("paired", "partnered")).starts_with("Partnered with the design team"));
    assert!(
        text(&swap("Paired with the design team to ship", "")).starts_with("A React dashboard")
    );
    assert!(text(&swap("design team", "product team")).contains("with the product team"));
}

#[test]
fn edits_to_an_entry_taken_off_are_skipped() {
    let (bank, master) = setup();
    let mk = |op: Op| {
        Change::new(
            op,
            Origin::Suggested,
            String::new(),
            vec![],
            String::new(),
            1,
            "x".into(),
        )
    };
    let swap_out = mk(Op::SwapEntry {
        out: "tidepool".into(),
        entry: "pixel".into(),
        bullets: vec![],
    });
    let cut = mk(Op::Cut {
        bullet: "tidepool.2".into(),
    });
    let exclude = mk(Op::Exclude {
        entry: "acme".into(),
    });
    let rewrite = mk(Op::Rewrite {
        bullet: "acme.1".into(),
        text: "x".into(),
    });
    let r = apply(&bank, &master, &[&cut, &swap_out, &rewrite, &exclude]).unwrap();
    assert!(!on(&r, "tidepool") && on(&r, "pixel") && !on(&r, "acme"));
    // The work section held only acme, so it goes with it.
    assert!(r.sections.iter().all(|s| s.id != "work"));
}

#[test]
fn candidates_rank_by_requirement_overlap() {
    let (bank, master) = setup();
    // kiln: Rust (priority 1, required: 4.5) + Docker (1); library: "help-desk", a near form of Help desk (0.5);
    // pixel: nothing.
    assert_eq!(
        bank_candidates(&bank, &master, &req(), 5),
        ["kiln", "library"]
    );
    assert_eq!(bank_candidates(&bank, &master, &req(), 1), ["kiln"]);
    let m = migrate_v1(&serde_json::from_str(V1).unwrap());
    assert!(bank_candidates(&m.bank, &m.master, &req(), 5).is_empty());
}
