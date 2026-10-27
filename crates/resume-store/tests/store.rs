//! The store as the app sees it, on synthetic data: the Jane Doe bank, a fixture posting's requirements and
//! suggestions, and made-up applications.

use resume_core::change::{Decision, DraftEvent, Mode};
use resume_core::feed::FeedJob;
use resume_core::salary::{Period, Salary};
use resume_core::story::{Story, StoryKind, StorySource, StoryTheme};
use resume_core::suggestions::Suggestions;
use resume_core::{Bank, Master, Posting, Requirements, Resume, migrate_v1};
use resume_store::{
    AppStatus, Application, Dig, DigAnswers, FeedState, LogoFound, LogoSource, NewAnalysis,
    NewFeedJob, PracticeRow, Prep, Rev, Round, RoundState, StatusChange, Store, StoreError,
    applications_csv, parse_ats_fit_csv,
};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");
const REQ: &str = include_str!("../../../fixtures/score/jane-doe-swe.requirements.json");
const SUGGESTIONS: &str =
    include_str!("../../../fixtures/suggestions/jane-doe/northwind-jr-swe.json");

fn jane() -> (Bank, Master) {
    let v1: Resume = serde_json::from_str(V1).unwrap();
    let m = migrate_v1(&v1);
    (m.bank, m.master)
}

fn requirements() -> Requirements {
    serde_json::from_str(REQ).unwrap()
}

fn suggestions() -> Suggestions {
    serde_json::from_str(SUGGESTIONS).unwrap()
}

fn posting(text: &str) -> Posting {
    Posting::pasted(text.to_owned())
}

fn decide(key: &str) -> DraftEvent {
    DraftEvent::Decide {
        key: key.into(),
        decision: Some(Decision::Off),
    }
}

/// A store with Jane's bank and master, one posting, and an analysis of it under `key`.
struct Fixture {
    store: Store,
    bank_rev: Rev,
    master_id: String,
    master_rev: Rev,
    posting_id: String,
}

impl Fixture {
    fn new() -> Self {
        let store = Store::open_in_memory().unwrap();
        let (bank, master) = jane();
        let bank_rev = store.put_bank(&bank).unwrap();
        let master_rev = store.put_master(&master).unwrap();
        let posting_id = store
            .put_posting(&posting("Backend engineer. Rust, Postgres."))
            .unwrap();
        Self {
            store,
            bank_rev,
            master_id: master.id,
            master_rev,
            posting_id,
        }
    }

    fn analysis(&self, key: &str, model: &str, s: &Suggestions) -> resume_store::AnalysisRow {
        self.store
            .put_analysis(&NewAnalysis {
                posting_id: &self.posting_id,
                bank_rev: &self.bank_rev,
                master_id: &self.master_id,
                master_rev: &self.master_rev,
                mode: Mode::MasterOnly,
                key,
                model,
                suggestions: s,
                latency_ms: 15_000,
            })
            .unwrap()
    }
}

fn app(id: &str, date: &str, company: &str) -> Application {
    Application {
        id: id.into(),
        posting_id: None,
        analysis_id: None,
        date: date.into(),
        company: company.into(),
        role: "Software Engineer".into(),
        url: None,
        status: AppStatus::Saved,
        now: Some(61.0),
        projected: Some(78.4),
        on_file: None,
        resume_path: None,
        cover_path: None,
        notes: String::new(),
        updated_at: String::new(),
    }
}

#[test]
fn revisions_dedupe_and_track_the_current_one() {
    let store = Store::open_in_memory().unwrap();
    let (bank, master) = jane();
    assert!(store.bank().unwrap().is_none());

    let first = store.put_bank(&bank).unwrap();
    assert_eq!(store.put_bank(&bank).unwrap(), first);
    assert_eq!(first.0.len(), 64);
    assert_eq!(first.to_string(), first.0[..12]);

    let mut renamed = bank.clone();
    renamed.name = "Jane Doe-Smith".into();
    let second = store.put_bank(&renamed).unwrap();
    assert_ne!(second, first);
    assert_eq!(store.bank().unwrap().unwrap(), (second.clone(), renamed));

    // Re-importing the old document makes its existing revision current again.
    assert_eq!(store.put_bank(&bank).unwrap(), first);
    assert_eq!(store.bank().unwrap().unwrap(), (first, bank));
    assert!(store.bank_updated_at().unwrap().is_some());

    let m1 = store.put_master(&master).unwrap();
    assert_eq!(store.put_master(&master).unwrap(), m1);
    let mut retitled = master.clone();
    retitled.title = "Backend".into();
    let m2 = store.put_master(&retitled).unwrap();
    assert_ne!(m2, m1);
    assert_eq!(
        store.masters().unwrap(),
        vec![(m2.clone(), retitled.clone())]
    );
    assert_eq!(store.master(&master.id).unwrap(), Some((m2, retitled)));
    assert!(store.master("nope").unwrap().is_none());

    let mut other = master.clone();
    other.id = "design".into();
    other.title = "Art direction".into();
    store.put_master(&other).unwrap();
    let titles: Vec<String> = store
        .masters()
        .unwrap()
        .into_iter()
        .map(|(_, m)| m.title)
        .collect();
    assert_eq!(titles, ["Art direction", "Backend"]);
}

#[test]
fn postings_dedupe_by_text_and_list_newest_first() {
    let store = Store::open_in_memory().unwrap();
    let id = store.put_posting(&posting("Same text")).unwrap();
    let other = store.put_posting(&posting("Other text")).unwrap();
    assert_ne!(id, other);

    let mut reread = posting("Same text");
    reread.company = Some("Acme".into());
    assert_eq!(store.put_posting(&reread).unwrap(), id);

    let rows = store.postings().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, id, "a re-read moves it to the top");
    assert_eq!(rows[0].posting.company.as_deref(), Some("Acme"));
    assert!(rows[0].updated_at > rows[0].created_at);
    assert_eq!(
        store.posting(&other).unwrap().unwrap().posting.text,
        "Other text"
    );
    assert!(store.posting("missing").unwrap().is_none());
}

#[test]
fn your_salary_wins_over_the_postings_and_survives_a_reread() {
    let store = Store::open_in_memory().unwrap();
    let text = "Backend engineer.\nSalary: $80,000 to $105,000 per year";
    let id = store.put_posting(&posting(text)).unwrap();
    let salary = |id: &str| store.posting(id).unwrap().unwrap().salary();
    let year = |lo: u32, hi: u32| Salary::new(Some(lo * 100), Some(hi * 100), "USD", Period::Year);
    assert_eq!(salary(&id), year(80_000, 105_000), "read from the text");

    let mut sourced = posting(text);
    sourced.salary = year(90_000, 110_000);
    store.put_posting(&sourced).unwrap();
    assert_eq!(
        salary(&id),
        year(90_000, 110_000),
        "the source's own fields beat the text"
    );

    let yours = year(95_000, 120_000).unwrap();
    store.set_posting_salary(&id, Some(&yours)).unwrap();
    store.put_posting(&sourced).unwrap();
    assert_eq!(salary(&id), Some(yours), "yours stays through a re-read");

    store.set_posting_salary(&id, None).unwrap();
    assert_eq!(
        salary(&id),
        None,
        "removed, the posting's doesn't come back"
    );

    assert!(matches!(
        store.set_posting_salary("missing", None),
        Err(StoreError::NotFound { .. })
    ));
}

#[test]
fn requirements_by_key_and_model() {
    let f = Fixture::new();
    let req = requirements();
    assert!(f.store.requirements("k", "sonnet").unwrap().is_none());
    assert!(
        f.store
            .latest_requirements(&f.posting_id)
            .unwrap()
            .is_none()
    );

    f.store
        .put_requirements(&f.posting_id, "k", "sonnet", &req, 6_000)
        .unwrap();
    assert_eq!(
        f.store.requirements("k", "sonnet").unwrap(),
        Some(req.clone())
    );
    assert!(f.store.requirements("k", "opus").unwrap().is_none());

    let mut newer = req.clone();
    newer.role = "Platform Engineer".into();
    f.store
        .put_requirements(&f.posting_id, "k2", "opus", &newer, 9_000)
        .unwrap();
    assert_eq!(
        f.store.latest_requirements(&f.posting_id).unwrap(),
        Some(newer)
    );
}

#[test]
fn analyses_by_key_and_model() {
    let f = Fixture::new();
    let s = suggestions();
    let a = f.analysis("key-1", "sonnet", &s);
    assert_eq!(a.created_at, a.opened_at);
    assert_eq!(a.projected, None);

    let found = f.store.analysis("key-1", "sonnet").unwrap().unwrap();
    assert_eq!(found, a);
    assert!(f.store.analysis("key-1", "opus").unwrap().is_none());
    assert!(f.store.analysis("key-2", "sonnet").unwrap().is_none());

    // A fresh re-run under the same key is a second analysis; the lookup returns the newest.
    let mut rerun = s.clone();
    rerun.headline = "Second opinion".into();
    let b = f.analysis("key-1", "sonnet", &rerun);
    assert_ne!(a.id, b.id);
    assert_eq!(
        f.store.analysis("key-1", "sonnet").unwrap().unwrap().id,
        b.id
    );
    assert_eq!(
        f.store.latest_analysis(&f.posting_id).unwrap().unwrap().id,
        b.id
    );

    f.store.touch_analysis(&a.id).unwrap();
    assert_eq!(
        f.store.latest_analysis(&f.posting_id).unwrap().unwrap().id,
        a.id
    );
    let ids: Vec<String> = f
        .store
        .analyses(&f.posting_id)
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(ids, [a.id.clone(), b.id.clone()]);

    f.store.set_projected(&b.id, 83.5).unwrap();
    let b = f.store.analysis_by_id(&b.id).unwrap().unwrap();
    assert_eq!(b.projected, Some(83.5));
    assert_eq!(b.suggestions, rerun);
    assert_eq!(b.mode, Mode::MasterOnly);
    assert!(matches!(
        f.store.touch_analysis("missing"),
        Err(StoreError::NotFound { .. })
    ));
}

#[test]
fn draft_push_after_undo_drops_the_redo_tail() {
    let f = Fixture::new();
    let a = f.analysis("key", "sonnet", &suggestions());
    assert_eq!(f.store.draft(&a.id).unwrap(), Default::default());

    for k in ["a", "b", "c"] {
        f.store.push_draft_event(&a.id, &decide(k)).unwrap();
    }
    let log = f.store.set_draft_cursor(&a.id, 1).unwrap();
    assert_eq!((log.events.len(), log.cursor), (3, 1));
    assert_eq!(log.draft().decisions.keys().collect::<Vec<_>>(), ["a"]);

    // Redo is still possible until something new is pushed.
    assert_eq!(f.store.draft(&a.id).unwrap().events.len(), 3);
    let log = f.store.push_draft_event(&a.id, &decide("d")).unwrap();
    assert_eq!(log.events, [decide("a"), decide("d")]);
    assert_eq!(log.cursor, 2);
    assert_eq!(f.store.draft(&a.id).unwrap(), log);
    assert_eq!(log.draft().decisions.keys().collect::<Vec<_>>(), ["a", "d"]);
}

#[test]
fn draft_cursor_is_clamped() {
    let f = Fixture::new();
    let a = f.analysis("key", "sonnet", &suggestions());
    assert_eq!(f.store.set_draft_cursor(&a.id, 5).unwrap().cursor, 0);

    f.store.push_draft_event(&a.id, &decide("a")).unwrap();
    f.store.push_draft_event(&a.id, &decide("b")).unwrap();
    assert_eq!(f.store.set_draft_cursor(&a.id, 0).unwrap().cursor, 0);
    let log = f.store.set_draft_cursor(&a.id, 99).unwrap();
    assert_eq!(log.cursor, 2);
    assert_eq!(f.store.draft(&a.id).unwrap().cursor, 2);
    assert!(matches!(
        f.store.push_draft_event("missing", &decide("a")),
        Err(StoreError::NotFound { .. })
    ));
}

#[test]
fn deleting_a_posting_cascades_but_keeps_its_application() {
    let f = Fixture::new();
    f.store
        .put_requirements(&f.posting_id, "rk", "sonnet", &requirements(), 6_000)
        .unwrap();
    let a = f.analysis("sk", "sonnet", &suggestions());
    f.store.push_draft_event(&a.id, &decide("a")).unwrap();
    let mut applied = app("app-1", "2026-10-01", "Acme");
    applied.posting_id = Some(f.posting_id.clone());
    applied.analysis_id = Some(a.id.clone());
    f.store.put_application(&applied).unwrap();
    assert_eq!(
        f.store
            .application_for_posting(&f.posting_id)
            .unwrap()
            .unwrap()
            .id,
        "app-1"
    );

    let kept = f.store.put_posting(&posting("Another job")).unwrap();
    f.store.delete_posting(&f.posting_id).unwrap();

    assert!(f.store.posting(&f.posting_id).unwrap().is_none());
    assert!(f.store.requirements("rk", "sonnet").unwrap().is_none());
    assert!(f.store.analysis_by_id(&a.id).unwrap().is_none());
    assert!(f.store.latest_analysis(&f.posting_id).unwrap().is_none());
    assert_eq!(f.store.draft(&a.id).unwrap(), Default::default());
    assert!(f.store.posting(&kept).unwrap().is_some());

    let apps = f.store.applications().unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].company, "Acme");
    assert_eq!(
        (apps[0].posting_id.clone(), apps[0].analysis_id.clone()),
        (None, None)
    );
}

#[test]
fn applications_upsert_by_id_and_change_status() {
    let store = Store::open_in_memory().unwrap();
    store
        .put_application(&app("old", "2026-09-01", "Older"))
        .unwrap();
    let mut a = app("new", "2026-10-01", "Newer");
    store.put_application(&a).unwrap();
    a.on_file = Some(74.0);
    a.notes = "Recruiter call Tuesday".into();
    store.put_application(&a).unwrap();

    let apps = store.applications().unwrap();
    assert_eq!(
        apps.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
        ["new", "old"]
    );
    assert_eq!(apps[0].on_file, Some(74.0));
    assert_eq!(apps[0].notes, "Recruiter call Tuesday");
    assert!(!apps[0].updated_at.is_empty());

    let changed = store
        .set_application_status(&["old", "new"], AppStatus::Interviewing)
        .unwrap();
    assert_eq!(
        changed.iter().map(|a| (a.id.as_str(), a.status)).collect::<Vec<_>>(),
        [
            ("old", AppStatus::Interviewing),
            ("new", AppStatus::Interviewing)
        ]
    );
    assert!(store
        .applications()
        .unwrap()
        .iter()
        .all(|a| a.status == AppStatus::Interviewing));

    // One unknown id: none of them change.
    assert!(matches!(
        store.set_application_status(&["old", "missing"], AppStatus::Offer),
        Err(StoreError::NotFound { .. })
    ));
    assert_eq!(
        store.applications().unwrap()[1].status,
        AppStatus::Interviewing
    );
}

#[test]
fn status_changes_are_logged_with_their_time() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.status_history("a").unwrap().is_empty());
    let mut a = app("a", "2026-10-01", "Acme");
    store.put_application(&a).unwrap();
    a.notes = "Recruiter call Tuesday".into();
    store.put_application(&a).unwrap();
    store
        .set_application_status(&["a"], AppStatus::Saved)
        .unwrap();
    let first = store.status_history("a").unwrap();
    assert_eq!(
        first.iter().map(|c| c.status).collect::<Vec<_>>(),
        [AppStatus::Saved],
        "unchanged statuses log nothing"
    );

    let changed = store
        .set_application_status(&["a"], AppStatus::Interviewing)
        .unwrap();
    a.status = AppStatus::Offer;
    store.put_application(&a).unwrap();
    let history = store.status_history("a").unwrap();
    assert_eq!(
        history.iter().map(|c| c.status).collect::<Vec<_>>(),
        [AppStatus::Saved, AppStatus::Interviewing, AppStatus::Offer]
    );
    assert_eq!(history[1].at, changed[0].updated_at);
    assert!(history.windows(2).all(|w| w[0].at < w[1].at));

    // A failed batch logs nothing.
    assert!(
        store
            .set_application_status(&["a", "missing"], AppStatus::Rejected)
            .is_err()
    );
    assert_eq!(store.status_history("a").unwrap().len(), 3);
}

#[test]
fn imported_applications_log_their_status() {
    let store = Store::open_in_memory().unwrap();
    let mut a = app("x", "2026-10-05", "Acme");
    a.status = AppStatus::Applied;
    store.import_applications(&[a]).unwrap();
    let history = store.status_history("x").unwrap();
    assert_eq!(
        history,
        [StatusChange {
            status: AppStatus::Applied,
            at: store.applications().unwrap()[0].updated_at.clone()
        }]
    );
}

#[test]
fn sent_resumes_round_trip() {
    let store = Store::open_in_memory().unwrap();
    assert!(matches!(
        store.put_sent("a", "{}"),
        Err(StoreError::NotFound { .. })
    ));
    assert_eq!(store.sent("a").unwrap(), None);
    store
        .put_application(&app("a", "2026-10-01", "Acme"))
        .unwrap();
    assert_eq!(store.sent("a").unwrap(), None);
    store.put_sent("a", r#"{"name":"Jane"}"#).unwrap();
    store.put_sent("a", r#"{"name":"Jane Doe"}"#).unwrap();
    assert_eq!(
        store.sent("a").unwrap().as_deref(),
        Some(r#"{"name":"Jane Doe"}"#)
    );
    // Saving the application again keeps it.
    store
        .put_application(&app("a", "2026-10-01", "Acme"))
        .unwrap();
    assert!(store.sent("a").unwrap().is_some());
}

/// Synthetic legacy applications.csv rows, oldest first: quoted commas, an escaped quote, empty scores and files.
const ATS_FIT: &str = "\
date,company,role,fit_planned,fit_on_page,resume,cover_letter,posting
2026-09-30,Acme,Software Engineer,82,74,/Users/jane/Resumes/acme_swe_doe_resume.pdf,,https://example.com/jobs/1
2026-10-02,\"Widgets, Inc.\",\"Engineer, Platform\",90,,/Users/jane/Resumes/widgets_eng_doe_resume.pdf,/Users/jane/Resumes/widgets_cover.pdf,\"https://example.com/jobs?id=2&tags=a,b\"
2026-10-02,\"The \"\"Best\"\" Co\",Developer,,,,,
";

#[test]
fn ats_fit_csv_round_trips() {
    let apps = parse_ats_fit_csv(ATS_FIT);
    assert_eq!(apps.len(), 3);
    // Newest first, like the store lists them.
    let widgets = &apps[1];
    assert_eq!(widgets.company, "Widgets, Inc.");
    assert_eq!(widgets.role, "Engineer, Platform");
    assert_eq!(widgets.projected, Some(90.0));
    assert_eq!(widgets.on_file, None);
    assert_eq!(
        widgets.url.as_deref(),
        Some("https://example.com/jobs?id=2&tags=a,b")
    );
    assert_eq!(
        widgets.cover_path.as_deref(),
        Some("/Users/jane/Resumes/widgets_cover.pdf")
    );
    assert_eq!(apps[0].company, "The \"Best\" Co");
    assert_eq!(apps[0].resume_path, None);
    assert_eq!(apps[2].on_file, Some(74.0));
    assert!(
        apps.iter()
            .all(|a| a.status == AppStatus::Applied && a.posting_id.is_none())
    );

    // The legacy eight columns come back exactly, with the app's two after them.
    let mut expected = ATS_FIT.replacen("posting\n", "posting,status,fit_now\n", 1);
    expected = expected
        .lines()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                format!("{l}\n")
            } else {
                format!("{l},applied,\n")
            }
        })
        .collect();
    let written = applications_csv(&apps);
    assert_eq!(written, expected);

    // The app's own file reads back to the same applications, and through the store to the same file.
    let reread = parse_ats_fit_csv(&written);
    let strip = |a: &[Application]| {
        a.iter()
            .map(|a| Application {
                id: String::new(),
                updated_at: String::new(),
                ..a.clone()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(strip(&reread), strip(&apps));

    let store = Store::open_in_memory().unwrap();
    store.import_applications(&apps).unwrap();
    assert_eq!(applications_csv(&store.applications().unwrap()), written);
}

#[test]
fn csv_rounds_scores_and_writes_status() {
    let mut a = app("x", "2026-10-05", "Acme");
    a.status = AppStatus::Offer;
    a.on_file = Some(70.5);
    a.notes = "line one\nline two".into();
    let csv = applications_csv(&[a]);
    assert_eq!(
        csv.lines().nth(1),
        Some("2026-10-05,Acme,Software Engineer,78,71,,,,offer,61")
    );
    assert!(parse_ats_fit_csv("not,a,log\n1,2,3\n").is_empty());
    assert!(parse_ats_fit_csv("").is_empty());
}

#[test]
fn settings_and_reopening_a_file() {
    let dir = std::env::temp_dir().join(format!("resume-store-{}", std::process::id()));
    let path = dir.join("nested").join("gliide.sqlite");
    {
        let store = Store::open(&path).unwrap();
        assert_eq!(store.setting::<String>("theme").unwrap(), None);
        store.put_setting("theme", &"dark").unwrap();
        store.put_setting("theme", &"system").unwrap();
        store.put_bank(&jane().0).unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(
        store.setting::<String>("theme").unwrap().as_deref(),
        Some("system")
    );
    assert!(store.bank().unwrap().is_some());
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A store written before cover letters (schema version 1) opens with its data and gains `covers`.
#[test]
fn migrating_to_covers_keeps_the_data() {
    let dir = std::env::temp_dir().join(format!("resume-store-v1-{}", std::process::id()));
    let path = dir.join("gliide.sqlite");
    let (bank, master) = jane();
    let (posting_id, analysis_id) = {
        let store = Store::open(&path).unwrap();
        let bank_rev = store.put_bank(&bank).unwrap();
        let master_rev = store.put_master(&master).unwrap();
        let posting_id = store.put_posting(&posting("Backend engineer.")).unwrap();
        let a = store
            .put_analysis(&NewAnalysis {
                posting_id: &posting_id,
                bank_rev: &bank_rev,
                master_id: &master.id,
                master_rev: &master_rev,
                mode: Mode::MasterOnly,
                key: "k",
                model: "sonnet",
                suggestions: &suggestions(),
                latency_ms: 15_000,
            })
            .unwrap();
        store.push_draft_event(&a.id, &decide("b1")).unwrap();
        store
            .put_application(&Application {
                posting_id: Some(posting_id.clone()),
                ..app("a1", "2026-10-01", "Acme")
            })
            .unwrap();
        store.put_setting("theme", &"dark").unwrap();
        (posting_id, a.id)
    };
    // Back to version 1, as the M4 app left it.
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "DROP TABLE dig_answers; DROP TABLE digs; DROP TABLE stories;
         DROP TABLE practice; DROP TABLE preps; DROP TABLE rounds; DROP TABLE status_changes;
         ALTER TABLE applications DROP COLUMN sent;
         DROP TABLE places; DROP TABLE logo_lookups; DROP TABLE feed; DROP TABLE covers; DROP TABLE companies;
         ALTER TABLE postings DROP COLUMN salary; PRAGMA user_version = 1;",
    )
    .unwrap();
    drop(conn);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.bank().unwrap().unwrap().1, bank);
    assert_eq!(store.master(&master.id).unwrap().unwrap().1, master);
    let row = store.posting(&posting_id).unwrap().unwrap();
    assert_eq!(row.your_salary, None, "no salary of yours before version 4");
    assert_eq!(
        store.latest_analysis(&posting_id).unwrap().unwrap().id,
        analysis_id
    );
    let log = store.draft(&analysis_id).unwrap();
    assert_eq!((log.events, log.cursor), (vec![decide("b1")], 1));
    assert_eq!(store.applications().unwrap()[0].company, "Acme");
    assert_eq!(
        store.setting::<String>("theme").unwrap().as_deref(),
        Some("dark")
    );
    assert_eq!(store.cover(&analysis_id).unwrap(), None);
    let paragraphs = vec!["One.".to_owned(), "Two.".to_owned(), "Three.".to_owned()];
    let first = store
        .put_cover(
            &analysis_id,
            "Dear Acme Hiring Team,",
            &paragraphs,
            "Sincerely,",
        )
        .unwrap();
    let edited = store
        .put_cover(&analysis_id, "Dear Ms. Lee,", &paragraphs, "Sincerely,")
        .unwrap();
    assert_eq!(
        (edited.id, edited.greeting.as_str()),
        (first.id, "Dear Ms. Lee,")
    );
    assert_eq!(
        store.cover(&analysis_id).unwrap().unwrap().paragraphs,
        paragraphs
    );
    drop(store);
    let version: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 11);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A new logo replaces the old under a new stamp (the UI's URLs carry it, so no stale image shows); removing one
/// leaves the others.
#[test]
fn logos_replace_and_remove() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.logo("acme").unwrap(), None);
    let first = store.put_logo("acme", b"<svg/>", "image/svg+xml").unwrap();
    store.put_logo("northwind", b"png", "image/png").unwrap();
    let second = store.put_logo("acme", b"png2", "image/png").unwrap();
    assert_ne!(first, second);
    let logo = store.logo("acme").unwrap().unwrap();
    assert_eq!(
        (logo.bytes.as_slice(), logo.media_type.as_str(), logo.updated_at.as_str()),
        (b"png2".as_slice(), "image/png", second.as_str())
    );
    store.remove_logo("acme").unwrap();
    store.remove_logo("acme").unwrap();
    let keys: Vec<String> = store.logos().unwrap().into_iter().map(|(k, _)| k).collect();
    assert_eq!(keys, ["northwind"]);
}

/// A fetched logo fills a company without one and replaces a fetched one, never yours; yours replaces a fetched one.
#[test]
fn fetched_logos_never_replace_yours() {
    let store = Store::open_in_memory().unwrap();
    assert!(
        store
            .put_fetched_logo("acme", b"fh", "image/png", LogoSource::Freehire)
            .unwrap()
    );
    assert!(
        store
            .put_fetched_logo("acme", b"site", "image/png", LogoSource::Site)
            .unwrap()
    );
    let fetched = store.logo("acme").unwrap().unwrap();
    assert_eq!(
        (fetched.bytes.as_slice(), fetched.source),
        (b"site".as_slice(), LogoSource::Site)
    );

    let yours = store.put_logo("acme", b"<svg/>", "image/svg+xml").unwrap();
    assert!(
        !store
            .put_fetched_logo("acme", b"fh2", "image/png", LogoSource::Freehire)
            .unwrap()
    );
    let logo = store.logo("acme").unwrap().unwrap();
    assert_eq!(
        (
            logo.bytes.as_slice(),
            logo.media_type.as_str(),
            logo.updated_at.as_str(),
            logo.source
        ),
        (
            b"<svg/>".as_slice(),
            "image/svg+xml",
            yours.as_str(),
            LogoSource::Yours
        )
    );
}

/// The last lookup per company, replaced by the next.
#[test]
fn logo_lookups_keep_the_last() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.logo_lookup("acme").unwrap(), None);
    store.put_logo_lookup("acme", LogoFound::Nothing).unwrap();
    let first = store.logo_lookup("acme").unwrap().unwrap();
    assert_eq!(first.found, LogoFound::Nothing);
    store.put_logo_lookup("acme", LogoFound::Removed).unwrap();
    let last = store.logo_lookup("acme").unwrap().unwrap();
    assert_eq!(last.found, LogoFound::Removed);
    assert!(last.tried_at > first.tried_at);
}

/// The last lookup per geocoding query, replaced by the next; nothing found stored as `None`.
#[test]
fn place_lookups_keep_the_last() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.place_lookup("plano, tx").unwrap(), None);
    store.put_place_lookup("plano, tx", None).unwrap();
    let first = store.place_lookup("plano, tx").unwrap().unwrap();
    assert_eq!(first.json, None);
    store
        .put_place_lookup("plano, tx", Some(r#"{"name":"Plano"}"#))
        .unwrap();
    let last = store.place_lookup("plano, tx").unwrap().unwrap();
    assert_eq!(last.json.as_deref(), Some(r#"{"name":"Plano"}"#));
    assert!(last.tried_at > first.tried_at);
}

/// Logos set before fetched logos existed (schema version 5) stay yours.
#[test]
fn migrating_to_fetched_logos_keeps_yours() {
    let dir = std::env::temp_dir().join(format!("resume-store-v5-{}", std::process::id()));
    let path = dir.join("gliide.sqlite");
    Store::open(&path)
        .unwrap()
        .put_logo("acme", b"png", "image/png")
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "DROP TABLE dig_answers; DROP TABLE digs; DROP TABLE stories;
         DROP TABLE practice; DROP TABLE preps; DROP TABLE rounds; DROP TABLE status_changes;
         ALTER TABLE applications DROP COLUMN sent;
         DROP TABLE places; DROP TABLE logo_lookups; ALTER TABLE companies DROP COLUMN source; PRAGMA user_version = 5;",
    )
    .unwrap();
    drop(conn);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.logo("acme").unwrap().unwrap().source, LogoSource::Yours);
    assert!(
        !store
            .put_fetched_logo("acme", b"fh", "image/png", LogoSource::Freehire)
            .unwrap()
    );
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Applications from before interview prep (schema version 7) have no history or sent resume until they change.
#[test]
fn migrating_to_status_changes_keeps_the_applications() {
    let dir = std::env::temp_dir().join(format!("resume-store-v7-{}", std::process::id()));
    let path = dir.join("gliide.sqlite");
    Store::open(&path)
        .unwrap()
        .put_application(&app("a", "2026-10-01", "Acme"))
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "DROP TABLE dig_answers; DROP TABLE digs; DROP TABLE stories;
         DROP TABLE practice; DROP TABLE preps; DROP TABLE rounds; DROP TABLE status_changes;
         ALTER TABLE applications DROP COLUMN sent; PRAGMA user_version = 7;",
    )
    .unwrap();
    drop(conn);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.applications().unwrap()[0].company, "Acme");
    assert!(store.status_history("a").unwrap().is_empty());
    assert_eq!(store.sent("a").unwrap(), None);
    store
        .set_application_status(&["a"], AppStatus::Applied)
        .unwrap();
    assert_eq!(store.status_history("a").unwrap().len(), 1);
    drop(store);
    std::fs::remove_dir_all(&dir).unwrap();
}

fn round(id: &str, application: &str, starts_at: Option<&str>) -> Round {
    Round {
        id: id.into(),
        application_id: application.into(),
        kind: "technical".into(),
        starts_at: starts_at.map(Into::into),
        minutes: Some(45),
        with: "Priya Shah".into(),
        place: "https://zoom.us/j/1".into(),
        notes: String::new(),
        state: RoundState::Upcoming,
        updated_at: String::new(),
    }
}

#[test]
fn rounds_list_scheduled_first_and_upsert_by_id() {
    let store = Store::open_in_memory().unwrap();
    store
        .put_application(&app("a", "2026-10-01", "Acme"))
        .unwrap();
    // Offsets differ: the order is by the instant, not the text.
    store
        .put_round(&round("late", "a", Some("2026-11-03T14:00:00-05:00")))
        .unwrap();
    store
        .put_round(&round("early", "a", Some("2026-11-03T18:00:00+01:00")))
        .unwrap();
    store.put_round(&round("open", "a", None)).unwrap();
    let ids =
        |s: &Store| -> Vec<String> { s.rounds("a").unwrap().into_iter().map(|r| r.id).collect() };
    assert_eq!(ids(&store), ["early", "late", "open"]);

    let saved = store
        .put_round(&Round {
            minutes: None,
            with: String::new(),
            ..round("open", "a", Some("2026-11-01T09:00:00Z"))
        })
        .unwrap();
    assert_eq!((saved.minutes, saved.with.as_str()), (None, ""));
    assert!(!saved.updated_at.is_empty());
    assert_eq!(ids(&store), ["open", "early", "late"]);

    let removed = store.delete_round("late").unwrap().unwrap();
    assert_eq!(store.delete_round("late").unwrap(), None);
    assert_eq!(ids(&store), ["open", "early"]);
    // Undo: the same id saved again comes back.
    store.put_round(&removed).unwrap();
    assert_eq!(ids(&store), ["open", "early", "late"]);

    assert!(matches!(
        store.put_round(&round("x", "nope", None)),
        Err(StoreError::NotFound {
            what: "application",
            ..
        })
    ));
}

#[test]
fn cancelling_takes_only_rounds_not_over_and_restores_by_id() {
    let store = Store::open_in_memory().unwrap();
    store
        .put_application(&app("a", "2026-10-01", "Acme"))
        .unwrap();
    store
        .put_application(&app("b", "2026-10-02", "Globex"))
        .unwrap();
    store
        .put_round(&round("past", "a", Some("2001-01-01T09:00:00Z")))
        .unwrap();
    store
        .put_round(&round("soon", "a", Some("2999-01-01T09:00:00Z")))
        .unwrap();
    store.put_round(&round("open", "a", None)).unwrap();
    store
        .put_round(&round("other", "b", Some("2999-01-01T09:00:00Z")))
        .unwrap();

    let mut cancelled = store.cancel_upcoming_rounds(&["a"]).unwrap();
    cancelled.sort();
    assert_eq!(cancelled, ["open", "soon"]);
    assert_eq!(
        store.round("past").unwrap().unwrap().state,
        RoundState::Upcoming
    );
    assert_eq!(
        store.round("other").unwrap().unwrap().state,
        RoundState::Upcoming
    );
    assert!(store.cancel_upcoming_rounds(&["a"]).unwrap().is_empty());

    let restored = store
        .set_round_state(&["soon", "open"], RoundState::Upcoming)
        .unwrap();
    assert_eq!(
        restored
            .iter()
            .map(|r| (r.id.as_str(), r.state))
            .collect::<Vec<_>>(),
        [
            ("soon", RoundState::Upcoming),
            ("open", RoundState::Upcoming)
        ]
    );
    // An unknown id changes none of them.
    assert!(
        store
            .set_round_state(&["soon", "gone"], RoundState::Cancelled)
            .is_err()
    );
    assert_eq!(
        store.round("soon").unwrap().unwrap().state,
        RoundState::Upcoming
    );
}

#[test]
fn next_rounds_are_the_soonest_scheduled_not_ended() {
    let store = Store::open_in_memory().unwrap();
    for id in ["a", "b", "c"] {
        store.put_application(&app(id, "2026-10-01", id)).unwrap();
    }
    store
        .put_round(&round("a-past", "a", Some("2001-01-01T09:00:00Z")))
        .unwrap();
    store
        .put_round(&round("a-later", "a", Some("2999-03-01T09:00:00Z")))
        .unwrap();
    store
        .put_round(&round("a-next", "a", Some("2999-01-01T09:00:00+09:00")))
        .unwrap();
    store.put_round(&round("a-open", "a", None)).unwrap();
    store
        .put_round(&Round {
            state: RoundState::Cancelled,
            ..round("b-cancelled", "b", Some("2999-01-01T09:00:00Z"))
        })
        .unwrap();
    store.put_round(&round("c-open", "c", None)).unwrap();

    let next = store.next_rounds().unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next["a"].id, "a-next");
    assert_eq!(store.next_round("a").unwrap().unwrap().id, "a-next");
    assert_eq!(store.next_round("b").unwrap(), None);

    // A round under way (started a minute ago, an hour long when its length isn't known) is next until it ends.
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 60;
    store
        .put_round(&Round {
            minutes: None,
            ..round("c-now", "c", Some(&rfc3339_secs(started)))
        })
        .unwrap();
    assert_eq!(store.next_rounds().unwrap()["c"].id, "c-now");
}

/// RFC 3339 UTC for seconds since the Unix epoch (Howard Hinnant's civil-from-days).
fn rfc3339_secs(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let day_secs = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        day_secs / 3600,
        day_secs / 60 % 60,
        day_secs % 60
    )
}

/// Applications from before rounds (schema version 8) open with none and take new ones.
#[test]
fn migrating_to_rounds_keeps_the_applications() {
    let dir = std::env::temp_dir().join(format!("resume-store-v8-{}", std::process::id()));
    let path = dir.join("gliide.sqlite");
    Store::open(&path)
        .unwrap()
        .put_application(&app("a", "2026-10-01", "Acme"))
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "DROP TABLE dig_answers; DROP TABLE digs; DROP TABLE stories;
         DROP TABLE practice; DROP TABLE preps; DROP TABLE rounds; PRAGMA user_version = 8;",
    )
    .unwrap();
    drop(conn);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.applications().unwrap()[0].company, "Acme");
    assert!(store.rounds("a").unwrap().is_empty());
    store.put_round(&round("r", "a", None)).unwrap();
    assert_eq!(store.rounds("a").unwrap().len(), 1);
    drop(store);
    let version: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 11);
    std::fs::remove_dir_all(&dir).unwrap();
}

fn practice_row(id: &str, round: &str, question: &str) -> PracticeRow {
    PracticeRow {
        id: id.into(),
        round_id: Some(round.into()),
        story_id: None,
        question: question.into(),
        asked: "Walk me through the regression suite.".into(),
        answer: "I wrote 14 tests.".into(),
        feedback: r#"{"lines":[]}"#.into(),
        at: String::new(),
    }
}

#[test]
fn preps_replace_by_key_and_practice_lists_oldest_first() {
    let store = Store::open_in_memory().unwrap();
    store
        .put_application(&app("a", "2026-10-01", "Acme"))
        .unwrap();
    store.put_round(&round("r", "a", None)).unwrap();
    store.put_round(&round("s", "a", None)).unwrap();
    assert_eq!(store.prep("k").unwrap(), None);
    let prep = Prep {
        key: "k".into(),
        round_id: "r".into(),
        json: r#"{"focus":"Depth on testing"}"#.into(),
        model: "claude-sonnet-5-5".into(),
        at: String::new(),
    };
    let first = store.put_prep(&prep).unwrap();
    assert!(!first.at.is_empty());
    assert_eq!(store.prep("k").unwrap().unwrap(), first);
    // Again: the same key written anew.
    let again = store
        .put_prep(&Prep {
            json: r#"{"focus":"Depth on Rust"}"#.into(),
            ..prep.clone()
        })
        .unwrap();
    assert!(again.at > first.at);
    assert_eq!(store.prep("k").unwrap().unwrap().json, again.json);
    let other = store
        .put_prep(&Prep {
            key: "k2".into(),
            ..prep.clone()
        })
        .unwrap();
    assert_eq!(store.latest_prep("r").unwrap().unwrap().key, other.key);
    assert_eq!(store.latest_prep("s").unwrap(), None);
    assert!(matches!(
        store.put_prep(&Prep {
            round_id: "gone".into(),
            ..prep
        }),
        Err(StoreError::NotFound { what: "round", .. })
    ));

    store.add_practice(&practice_row("p1", "r", "q1")).unwrap();
    store.add_practice(&practice_row("p2", "s", "q1")).unwrap();
    store.add_practice(&practice_row("p3", "r", "q2")).unwrap();
    let ids = |round: &str| -> Vec<String> {
        store
            .practice(round)
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect()
    };
    assert_eq!(ids("r"), ["p1", "p3"]);
    assert_eq!(store.practice("r").unwrap()[1].question, "q2");
    assert!(
        store
            .add_practice(&practice_row("p4", "gone", "q1"))
            .is_err()
    );

    // Removing a round removes its prep and practice.
    store.delete_round("r").unwrap();
    assert_eq!(store.prep("k").unwrap(), None);
    assert!(ids("r").is_empty());
    assert_eq!(ids("s"), ["p2"]);
}

/// Rounds from before prep (schema version 9) keep their place and take preps and practice.
#[test]
fn migrating_to_practice_keeps_the_rounds() {
    let dir = std::env::temp_dir().join(format!("resume-store-v9-{}", std::process::id()));
    let path = dir.join("gliide.sqlite");
    {
        let store = Store::open(&path).unwrap();
        store
            .put_application(&app("a", "2026-10-01", "Acme"))
            .unwrap();
        store.put_round(&round("r", "a", None)).unwrap();
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "DROP TABLE dig_answers; DROP TABLE digs; DROP TABLE stories;
         DROP TABLE practice; DROP TABLE preps; PRAGMA user_version = 9;",
    )
    .unwrap();
    drop(conn);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.rounds("a").unwrap()[0].id, "r");
    assert!(store.practice("r").unwrap().is_empty());
    store.add_practice(&practice_row("p", "r", "q1")).unwrap();
    assert_eq!(store.practice("r").unwrap().len(), 1);
    drop(store);
    let version: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 11);
    std::fs::remove_dir_all(&dir).unwrap();
}

fn story(id: &str, updated_at: &str) -> Story {
    Story {
        id: id.into(),
        title: "Caught 14 regressions before release".into(),
        entry: Some("acme".into()),
        bullets: vec!["acme.2".into()],
        facts: vec![],
        situation: "Releases kept breaking.".into(),
        task: "I owned the tests.".into(),
        action: "I wrote integration tests.".into(),
        result: "They caught 14 regressions.".into(),
        skills: vec![],
        themes: vec![StoryTheme::Impact],
        source: StorySource {
            kind: StoryKind::Edit,
            at: updated_at.into(),
            from: None,
        },
        updated_at: updated_at.into(),
    }
}

#[test]
fn stories_list_newest_first_and_come_back_after_removal() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.stories().unwrap().is_empty());
    store.put_story(&story("a", "2026-10-01T10:00:00Z")).unwrap();
    store.put_story(&story("b", "2026-10-02T10:00:00Z")).unwrap();
    let ids = |store: &Store| -> Vec<String> {
        store.stories().unwrap().into_iter().map(|s| s.id).collect()
    };
    assert_eq!(ids(&store), ["b", "a"]);
    // Editing one moves it first.
    let edited = Story {
        title: "Tests".into(),
        ..story("a", "2026-10-03T10:00:00Z")
    };
    store.put_story(&edited).unwrap();
    assert_eq!(ids(&store), ["a", "b"]);
    assert_eq!(store.story("a").unwrap().unwrap(), edited);
    // Removed, then put back by its id (Undo); its tellings find it again.
    let mut told = practice_row("t", "", "a");
    told.round_id = None;
    told.story_id = Some("a".into());
    store.add_practice(&told).unwrap();
    assert_eq!(store.delete_story("a").unwrap(), Some(edited.clone()));
    assert_eq!(store.delete_story("a").unwrap(), None);
    assert_eq!(ids(&store), ["b"]);
    assert_eq!(store.story_practice("a").unwrap().len(), 1);
    store.put_story(&edited).unwrap();
    assert_eq!(ids(&store), ["a", "b"]);
}

#[test]
fn practice_can_tell_a_story_alone_or_for_a_round() {
    let store = Store::open_in_memory().unwrap();
    store
        .put_application(&app("a", "2026-10-01", "Acme"))
        .unwrap();
    store.put_round(&round("r", "a", None)).unwrap();
    let mut alone = practice_row("p1", "", "s");
    alone.round_id = None;
    alone.story_id = Some("s".into());
    let stored = store.add_practice(&alone).unwrap();
    assert_eq!(stored.round_id, None);
    let mut for_round = practice_row("p2", "r", "q1");
    for_round.story_id = Some("s".into());
    store.add_practice(&for_round).unwrap();
    store.add_practice(&practice_row("p3", "r", "q2")).unwrap();
    let ids = |rows: Vec<PracticeRow>| -> Vec<String> { rows.into_iter().map(|p| p.id).collect() };
    assert_eq!(ids(store.story_practice("s").unwrap()), ["p1", "p2"]);
    assert_eq!(ids(store.told_practice().unwrap()), ["p2", "p1"]);
    assert_eq!(ids(store.practice("r").unwrap()), ["p2", "p3"]);
    assert_eq!(store.practice_row("p1").unwrap().unwrap().story_id.as_deref(), Some("s"));
    assert_eq!(store.practice_row("nope").unwrap(), None);
    // A round removed takes its practice; a story told alone stays.
    store.delete_round("r").unwrap();
    assert_eq!(ids(store.story_practice("s").unwrap()), ["p1"]);
}

#[test]
fn digs_replace_by_key_and_answers_are_kept() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.dig("k").unwrap(), None);
    let d = Dig {
        key: "k".into(),
        entry: "acme".into(),
        json: r#"{"read":"3 bullets"}"#.into(),
        model: "claude-sonnet-5-5".into(),
        at: String::new(),
    };
    let first = store.put_dig(&d).unwrap();
    assert!(!first.at.is_empty());
    assert_eq!(store.dig("k").unwrap().unwrap(), first);
    let again = store
        .put_dig(&Dig {
            json: r#"{"read":"4 bullets"}"#.into(),
            ..d
        })
        .unwrap();
    assert_eq!(store.dig("k").unwrap().unwrap().json, again.json);
    let a = store
        .add_dig_answers(&DigAnswers {
            id: "c".into(),
            entry: "acme".into(),
            json: "[]".into(),
            at: String::new(),
        })
        .unwrap();
    assert_eq!(store.dig_answers("c").unwrap().unwrap(), a);
    assert_eq!(store.dig_answers("x").unwrap(), None);
}

/// Practice from before stories (schema version 10) keeps its rows, now with no story, and takes tellings alone.
#[test]
fn migrating_to_stories_keeps_the_practice() {
    let dir = std::env::temp_dir().join(format!("resume-store-v10-{}", std::process::id()));
    let path = dir.join("gliide.sqlite");
    {
        let store = Store::open(&path).unwrap();
        store
            .put_application(&app("a", "2026-10-01", "Acme"))
            .unwrap();
        store.put_round(&round("r", "a", None)).unwrap();
    }
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "DROP TABLE dig_answers; DROP TABLE digs; DROP TABLE stories; DROP TABLE practice;
         CREATE TABLE practice (
             id TEXT PRIMARY KEY,
             round_id TEXT NOT NULL REFERENCES rounds (id) ON DELETE CASCADE,
             question TEXT NOT NULL,
             asked TEXT NOT NULL,
             answer TEXT NOT NULL,
             feedback TEXT NOT NULL,
             at TEXT NOT NULL
         );
         CREATE INDEX practice_round ON practice (round_id);
         INSERT INTO practice VALUES ('p1', 'r', 'q1', 'Why Acme?', 'I like widgets.', '{}', '2026-10-02T10:00:00Z');
         INSERT INTO practice VALUES ('p2', 'r', 'q2', 'Tests?', 'I wrote 14.', '{}', '2026-10-02T10:00:00Z');
         PRAGMA user_version = 10;",
    )
    .unwrap();
    drop(conn);

    let store = Store::open(&path).unwrap();
    let rows = store.practice("r").unwrap();
    assert_eq!(
        rows.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        ["p1", "p2"]
    );
    assert_eq!(rows[1].answer, "I wrote 14.");
    assert_eq!(rows[0].story_id, None);
    assert_eq!(rows[0].round_id.as_deref(), Some("r"));
    let mut alone = practice_row("p3", "", "s");
    alone.round_id = None;
    alone.story_id = Some("s".into());
    store.add_practice(&alone).unwrap();
    store.put_story(&story("s", "2026-10-03T10:00:00Z")).unwrap();
    assert_eq!(store.stories().unwrap().len(), 1);
    // Removing the round still removes its practice.
    store.delete_round("r").unwrap();
    assert!(store.practice("r").unwrap().is_empty());
    assert_eq!(store.story_practice("s").unwrap().len(), 1);
    drop(store);
    let version: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 11);
    std::fs::remove_dir_all(&dir).unwrap();
}

fn feed_job(slug: &str, found_at: &str) -> NewFeedJob {
    NewFeedJob {
        job: FeedJob {
            origin: Default::default(),
            description: None,
            slug: slug.into(),
            title: "Junior Software Engineer".into(),
            company: "Acme".into(),
            company_slug: None,
            location: None,
            url: format!("https://acme.example/jobs/{slug}"),
            source: "workday".into(),
            external_id: None,
            skills: vec!["rust".into()],
            posted_at: None,
            found_at: found_at.into(),
            seniority: Some("junior".into()),
            employment_type: None,
            work_mode: None,
            years_min: None,
            salary: None,
        },
        state: FeedState::New,
        overlap: 1.0,
        reason: None,
    }
}

/// A later pull finding a job again leaves its row (and what was decided about it) as it is.
#[test]
fn adding_feed_jobs_ignores_known_slugs() {
    let store = Store::open_in_memory().unwrap();
    let a = feed_job("a", "2026-10-13T10:00:00Z");
    let b = feed_job("b", "2026-10-13T11:00:00Z");
    assert_eq!(store.add_feed_jobs(&[a.clone(), b.clone()]).unwrap(), 2);
    store
        .set_feed_state("a", FeedState::Skipped, Some("asks for 5 years"))
        .unwrap();
    let again = NewFeedJob {
        overlap: 9.0,
        ..a.clone()
    };
    let c = feed_job("c", "2026-10-13T12:00:00Z");
    assert_eq!(store.add_feed_jobs(&[again, c]).unwrap(), 1);
    let a_row = store.feed_row("a").unwrap().unwrap();
    assert_eq!(
        (a_row.state, a_row.overlap, a_row.reason.as_deref()),
        (FeedState::Skipped, 1.0, Some("asks for 5 years"))
    );
    assert_eq!(a_row.job, a.job);
    let slugs: Vec<String> = store
        .feed()
        .unwrap()
        .into_iter()
        .map(|r| r.job.slug)
        .collect();
    assert_eq!(slugs, ["c", "b", "a"]);
}

#[test]
fn deleting_a_feed_posting_clears_the_link() {
    let store = Store::open_in_memory().unwrap();
    store
        .add_feed_jobs(&[feed_job("a", "2026-10-13T10:00:00Z")])
        .unwrap();
    let id = store
        .put_posting(&posting("Junior Software Engineer at Acme."))
        .unwrap();
    store.set_feed_posting("a", &id).unwrap();
    assert_eq!(
        store.feed_row("a").unwrap().unwrap().posting_id,
        Some(id.clone())
    );
    store.delete_posting(&id).unwrap();
    let row = store.feed_row("a").unwrap().unwrap();
    assert_eq!(row.posting_id, None);
    assert!(matches!(
        store.set_feed_posting("missing", &id),
        Err(StoreError::NotFound { .. })
    ));
}

#[test]
fn feed_postings_stay_out_of_the_lists_until_opened() {
    let store = Store::open_in_memory().unwrap();
    store
        .add_feed_jobs(&[
            feed_job("a", "2026-10-13T10:00:00Z"),
            feed_job("b", "2026-10-13T11:00:00Z"),
        ])
        .unwrap();
    let a = store.put_posting(&posting("Posting A")).unwrap();
    let b = store.put_posting(&posting("Posting B")).unwrap();
    store.set_feed_posting("a", &a).unwrap();
    store.set_feed_posting("b", &b).unwrap();
    assert_eq!(
        store.feed_unopened_postings().unwrap(),
        [a.clone(), b.clone()].into_iter().collect()
    );
    store.set_feed_opened(&a).unwrap();
    // A posting no feed row holds: nothing to mark.
    store.set_feed_opened("pasted-posting").unwrap();
    assert_eq!(
        store.feed_unopened_postings().unwrap(),
        [b].into_iter().collect()
    );
    assert!(store.feed_row("a").unwrap().unwrap().opened_at.is_some());
}

#[test]
fn feed_checks_count_from_a_time() {
    let store = Store::open_in_memory().unwrap();
    store
        .add_feed_jobs(&[
            feed_job("a", "2026-10-13T10:00:00Z"),
            feed_job("b", "2026-10-13T11:00:00Z"),
        ])
        .unwrap();
    let before = "2000-01-01T00:00:00-05:00";
    assert_eq!(store.feed_checked_since(before).unwrap(), 0);
    store
        .set_feed_checked("a", Some(62.5), Some("swe"))
        .unwrap();
    store.set_feed_checked("b", None, None).unwrap();
    let row = store.feed_row("a").unwrap().unwrap();
    assert_eq!(
        (row.state, row.score, row.master_id.as_deref()),
        (FeedState::Checked, Some(62.5), Some("swe"))
    );
    assert_eq!(store.feed_checked_since(before).unwrap(), 2);
    let at = row.checked_at.unwrap();
    assert_eq!(store.feed_checked_since(&at).unwrap(), 2);
    assert_eq!(store.feed_checked_since("2999-01-01T00:00:00Z").unwrap(), 0);
}
