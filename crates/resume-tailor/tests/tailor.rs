use std::path::Path;

use resume_core::change::{Change, Decision, Draft, Mode, Op, Origin, State};
use resume_core::changes::changes_of;
use resume_core::suggestions::Suggestions;
use resume_core::{Bank, Master, Requirements, migrate_v1};
use resume_render::{FontOptions, Renderer, Template};
use resume_tailor::{MAX_ROUNDS, Tailored, tailor};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");
const REQ: &str = include_str!("../../../fixtures/score/jane-doe-swe.requirements.json");
const TODAY: (u32, u32) = (2026, 10);

fn fixture() -> (Bank, Master) {
    let m = migrate_v1(&serde_json::from_str(V1).unwrap());
    (m.bank, m.master)
}

fn req() -> Requirements {
    serde_json::from_str(REQ).unwrap()
}

fn change(op: Op, priority: u8) -> Change {
    Change::new(
        op,
        Origin::Suggested,
        String::new(),
        Vec::new(),
        "why".into(),
        priority,
        "work".into(),
    )
}

fn yours(op: Op) -> Change {
    Change {
        origin: Origin::Yours,
        ..change(op, 1)
    }
}

fn rewrite(bullet: &str, text: &str) -> Op {
    Op::Rewrite {
        bullet: bullet.into(),
        text: text.into(),
    }
}

fn run(bank: &Bank, master: &Master, changes: &[Change], draft: &Draft) -> Tailored {
    let mut r = Renderer::new(&Template::classic_serif(), FontOptions::default());
    tailor(bank, master, &req(), changes, draft, &mut r, TODAY).unwrap()
}

fn on(changes: &[&Change]) -> Draft {
    Draft {
        decisions: changes
            .iter()
            .map(|c| (c.key.clone(), Decision::On))
            .collect(),
        ..Draft::default()
    }
}

const LONG: &str = "Wrote integration tests in GitHub Actions for the order API, catching 14 regressions before \
    release, and documented every one of them in the team wiki with steps to reproduce, the fix, the root cause, \
    and the monitoring that now catches the same class of bug before it reaches a customer in production";

#[test]
fn line_edits_keep_their_line_budget() {
    let (bank, master) = fixture();
    let short = change(
        rewrite(
            "acme.3",
            "Paired with designers to ship a React dashboard plant managers use daily",
        ),
        1,
    );
    let long = change(rewrite("acme.2", LONG), 1);
    let note = change(
        Op::NoteAdd {
            note: "state-u#1".into(),
            term: "Algorithms".into(),
        },
        2,
    );
    let mine = yours(rewrite("library.1", LONG));
    let forced = change(rewrite("library.2", LONG), 1);
    let changes = [short, long, note, mine, forced];
    let t = run(&bank, &master, &changes, &on(&[&changes[4]]));

    let state = |i: usize| &t.states[&changes[i].key];
    assert_eq!(state(0), &State::In);
    assert!(
        matches!(state(1), State::LeftOut { reason } if reason.starts_with("takes ") && reason.ends_with("not 1")),
        "{:?}",
        state(1)
    );
    assert_eq!(state(2), &State::In);
    // Yours stays in, with a warning; one you turned on is forced, with the same.
    assert_eq!(state(3), &State::In);
    assert!(t.warnings[&changes[3].key].starts_with("takes "));
    assert!(
        matches!(state(4), State::Forced { warning: Some(w) } if w.starts_with("takes ")),
        "{:?}",
        state(4)
    );
    // The rewrite left out is not on the page; the others are.
    let text = resume_core::resume_text(&t.resume);
    assert!(text.contains("plant managers use daily"));
    assert_eq!(text.matches("documented every one of them").count(), 2);
    assert_eq!(
        t.compiles.measure, 2,
        "one measure compile per fit (draft, ceiling)"
    );
    assert_eq!(
        t.after[&changes[0].key],
        "Paired with designers to ship a React dashboard plant managers use daily"
    );
}

#[test]
fn validator_flags_leave_out_and_bank_changes_start_off() {
    let (bank, mut master) = fixture();
    master.sections.retain(|s| s.id != "projects");
    let flagged = Change {
        flags: vec!["“40%” isn't in your resume".into()],
        ..change(
            rewrite("acme.3", "Cut dashboard load time 40% with React"),
            1,
        )
    };
    let forced_flag = Change {
        flags: vec!["“Kafka” isn't in your bank".into()],
        ..change(rewrite("acme.1", "Built a Rust and Kafka service"), 1)
    };
    let include = Change {
        section: "bank".into(),
        ..change(
            Op::Include {
                entry: "tidepool".into(),
                bullets: Vec::new(),
            },
            3,
        )
    };
    let changes = [flagged, forced_flag, include];

    let t = run(&bank, &master, &changes, &on(&[&changes[1]]));
    assert!(
        matches!(&t.states[&changes[0].key], State::LeftOut { reason } if reason.contains("40%")),
        "{:?}",
        t.states
    );
    assert_eq!(t.states[&changes[1].key], State::Forced { warning: None });
    assert_eq!(t.states[&changes[2].key], State::Off);
    // The ceiling takes every unflagged change, bank ones included.
    assert!(matches!(
        t.ceiling_states[&changes[0].key],
        State::LeftOut { .. }
    ));
    assert_eq!(t.ceiling_states[&changes[2].key], State::In);
    assert!(!resume_core::resume_text(&t.resume).contains("Tidepool"));

    let t = run(&bank, &master, &changes, &on(&[&changes[2]]));
    assert_eq!(t.states[&changes[2].key], State::In);
    assert!(resume_core::resume_text(&t.resume).contains("Tidepool"));
}

#[test]
fn a_skills_line_keeps_its_line_count_by_priority() {
    let (bank, master) = fixture();
    let items = [
        "Go",
        "Kotlin",
        "Swift",
        "C++",
        "C#",
        "Ruby",
        "Scala",
        "Haskell",
        "Elixir",
        "OCaml",
        "Zig",
        "JavaScript",
        "Bash",
        "Perl",
        "Lua",
    ];
    let changes: Vec<Change> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            change(
                Op::SkillAdd {
                    group: "languages".into(),
                    item: (*item).into(),
                },
                [3, 1, 2][i % 3],
            )
        })
        .collect();
    let t = run(&bank, &master, &changes, &Draft::default());
    let left: Vec<&Change> = changes
        .iter()
        .filter(|c| matches!(&t.states[&c.key], State::LeftOut { reason } if reason.starts_with("the Languages line would take 2 lines")))
        .collect();
    let kept: Vec<&Change> = changes
        .iter()
        .filter(|c| t.states[&c.key] == State::In)
        .collect();
    assert_eq!(left.len() + kept.len(), changes.len(), "{:?}", t.states);
    assert!(!left.is_empty() && !kept.is_empty(), "{:?}", t.states);
    let worst_kept = kept.iter().map(|c| c.priority).max().unwrap();
    let best_left = left.iter().map(|c| c.priority).min().unwrap();
    assert!(
        worst_kept <= best_left,
        "kept p{worst_kept}, left out p{best_left}"
    );
    let g = t
        .rendered
        .metrics
        .groups
        .iter()
        .find(|g| g.id == "languages")
        .unwrap();
    assert_eq!((g.lines, g.orig), (1, 1));
}

#[test]
fn the_page_loop_leaves_out_new_bullets_until_it_fits() {
    let (bank, master) = fixture();
    let mut changes: Vec<Change> = (0..14)
        .map(|i| Change {
            // Adds under one entry share a key otherwise.
            key: format!("add{i}"),
            ..change(
                Op::Add {
                    entry: "acme".into(),
                    text: format!("{i}: {LONG}"),
                },
                [1, 2, 3][i % 3],
            )
        })
        .collect();
    changes.push(yours(Op::Add {
        entry: "library".into(),
        text: "Trained six staff on the new ticket queue".into(),
    }));
    let t = run(&bank, &master, &changes, &Draft::default());
    assert_eq!(t.rendered.metrics.pages, 1);
    let left: Vec<&Change> = changes
        .iter()
        .filter(|c| matches!(&t.states[&c.key], State::LeftOut { reason } if reason == "the page would overflow"))
        .collect();
    assert!(!left.is_empty());
    let kept: Vec<&Change> = changes
        .iter()
        .filter(|c| t.states[&c.key] == State::In)
        .collect();
    assert!(kept.iter().any(|c| c.origin == Origin::Yours));
    // Least important first: nothing kept is less important than something left out (yours aside).
    let worst_kept = kept
        .iter()
        .filter(|c| c.origin == Origin::Suggested)
        .map(|c| c.priority)
        .max()
        .unwrap_or(0);
    assert!(left.iter().all(|c| c.priority >= worst_kept));
    assert!(t.compiles.fit <= 1 + 2 * MAX_ROUNDS);
}

#[test]
fn scores_now_projected_ceiling_and_points() {
    let (bank, master) = fixture();
    let c = change(
        Op::SkillAdd {
            group: "languages".into(),
            item: "C".into(),
        },
        2,
    );
    let mongo = change(
        Op::SkillAdd {
            group: "tools".into(),
            item: "MongoDB".into(),
        },
        2,
    );
    let changes = [c, mongo];
    let draft = Draft {
        decisions: [(changes[1].key.clone(), Decision::Off)].into(),
        ..Draft::default()
    };
    let t = run(&bank, &master, &changes, &draft);
    assert_eq!(t.states[&changes[1].key], State::Off);
    assert!(
        t.projected.total > t.now.total,
        "{} vs {}",
        t.projected.total,
        t.now.total
    );
    assert!(
        t.ceiling.total > t.projected.total,
        "{} vs {}",
        t.ceiling.total,
        t.projected.total
    );
    // Points: in, projected minus without it; off, with it minus projected.
    let c_points = t.points[&changes[0].key];
    let mongo_points = t.points[&changes[1].key];
    assert!(c_points > 0.0 && mongo_points > 0.0, "{:?}", t.points);
    assert!((t.projected.total - t.now.total - c_points).abs() < 0.11);
}

/// Recorded Suggest outputs for the jane-doe master, through `changes_of` and the fit check: every one fits its page.
#[test]
fn recorded_suggestions_fit_one_page() {
    let (bank, master) = fixture();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut seen = 0;
    for entry in std::fs::read_dir(root.join("suggestions/jane-doe")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let s: Suggestions =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let req: Requirements = serde_json::from_str(
            &std::fs::read_to_string(root.join("requirements/sonnet").join(&name)).unwrap(),
        )
        .unwrap();
        let changes = changes_of(&s, &bank, &master, &req, Mode::MasterOnly);
        let mut r = Renderer::new(&Template::classic_serif(), FontOptions::default());
        let t = tailor(
            &bank,
            &master,
            &req,
            &changes,
            &Draft::default(),
            &mut r,
            TODAY,
        )
        .unwrap();
        assert!(t.rendered.metrics.pages <= master.page_limit, "{name}");
        assert_eq!(t.states.len(), changes.len(), "{name}");
        assert!(
            t.projected.total >= t.now.total,
            "{name}: {} < {}",
            t.projected.total,
            t.now.total
        );
        seen += 1;
    }
    assert!(seen > 0);
}
