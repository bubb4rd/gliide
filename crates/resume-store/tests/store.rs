//! The store as the app sees it, on synthetic data: the Jane Doe bank, a fixture posting's requirements and
//! suggestions, and made-up applications.

use resume_core::change::{Decision, DraftEvent, Mode};
use resume_core::suggestions::Suggestions;
use resume_core::{Bank, Master, Posting, Requirements, Resume, migrate_v1};
use resume_store::{
    AppStatus, Application, NewAnalysis, Rev, Store, StoreError, applications_csv,
    parse_ats_fit_csv,
};

const V1: &str = include_str!("../../../fixtures/banks/jane-doe.v1.json");
const REQ: &str = include_str!("../../../fixtures/score/jane-doe-swe.requirements.json");
const SUGGESTIONS: &str =
    include_str!("../../../fixtures/suggestions/jane-doe/waymark-jr-swe.json");

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
        .set_application_status("old", AppStatus::Interviewing)
        .unwrap();
    assert_eq!(changed.status, AppStatus::Interviewing);
    assert_eq!(
        store.applications().unwrap()[1].status,
        AppStatus::Interviewing
    );
    assert!(matches!(
        store.set_application_status("missing", AppStatus::Offer),
        Err(StoreError::NotFound { .. })
    ));
}

/// Synthetic ATS Fit rows, oldest first: quoted commas, an escaped quote, empty scores and files.
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

    // ATS Fit's columns come back exactly, with the app's two after them.
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
    let path = dir.join("nested").join("easyshmeezy.sqlite");
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
