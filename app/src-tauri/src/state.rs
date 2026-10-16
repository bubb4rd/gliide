//! The app's state (`tauri::State<Arc<App>>`): the store, the settings, the open review, warm renderers, the model
//! calls running and the feed's run.
//!
//! Locks are `std::sync::Mutex`, never held across an `await`. Order, when one is taken under another: `review`, then
//! `renderers`, then `pages`; `store` is always taken last and released before any other lock; `feed` is taken alone.
//! `feed_lane` is the exception: a `tokio` mutex held across a feed check's awaits, so checks run one at a time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use resume_core::{Bank, Master, PageFacts, Resume, resolve};
use resume_llm::extract::DEFAULT_EXTRACT_MODEL;
use resume_llm::suggest::DEFAULT_SUGGEST_MODEL;
use resume_llm::{ClaudeCli, LlmError};
use resume_render::{FontOptions, Renderer};
use resume_store::{Rev, Store};
use tokio::task::AbortHandle;

use crate::dto::{Backdrop, FeedStatus, Mode, Progress, Settings, Stage, Theme};
use crate::error::Result;
use crate::export;
use crate::feed::FeedRun;
use crate::import::Pending;
use crate::review::Review;

/// The store file inside the app's data folder.
pub const STORE_FILE: &str = "gliide.sqlite";
/// The settings' key in the store.
const SETTINGS: &str = "settings";
/// The template every master gets unless it names another (the only one so far).
pub const DEFAULT_TEMPLATE: &str = "classic-serif";

/// What the app tells the UI: Tauri's events, by name.
pub enum Event<'a> {
    /// `progress`: a model call started for a job.
    Progress(&'a Progress),
    /// `feed`: a feed row or the feed's status changed.
    Feed(&'a FeedStatus),
}

/// How the app tells the UI (Tauri's events; a no-op or a log without the webview).
pub type Emit = Box<dyn Fn(Event<'_>) + Send + Sync>;

pub struct App {
    pub store: Mutex<Store>,
    pub settings: Mutex<Settings>,
    /// The open review: one at a time.
    pub review: Mutex<Option<Review>>,
    /// A resume file read and copied, waiting for you to add it to the bank: one at a time.
    pub import: Mutex<Option<Pending>>,
    /// One long-lived renderer per template, kept warm across reviews.
    pub renderers: Mutex<HashMap<String, Renderer>>,
    /// Each master as rendered, by bank and master revision: its resolved resume and page facts for the score now.
    pub pages: Mutex<HashMap<(Rev, Rev), MasterPage>>,
    /// Model calls running, by job (posting id): `cancel` aborts them.
    pub calls: Mutex<HashMap<String, AbortHandle>>,
    /// Counts `start_review` calls: only the newest may put its review in the slot.
    pub review_generation: AtomicU64,
    /// The feed's pull and checks (feed.rs).
    pub feed: Mutex<FeedRun>,
    /// Held while a feed job is checked: one at a time.
    pub feed_lane: tokio::sync::Mutex<()>,
    pub emit: Emit,
}

/// A master as rendered: what the score needs.
#[derive(Clone)]
pub struct MasterPage {
    pub resume: Resume,
    pub facts: PageFacts,
}

/// `m` locked, also after a panic in another holder (the data stays usable: every writer finishes its write).
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl App {
    /// The app over the store in `dir` (created when missing).
    pub fn open(dir: &Path, emit: Emit) -> Result<Self> {
        Self::new(Store::open(&dir.join(STORE_FILE))?, emit)
    }

    /// The app over `store`: settings loaded (defaults when none), and the rows of `applications.csv` in the export
    /// folder the store doesn't have imported (every write imports again first, so a failure here only waits).
    pub fn new(store: Store, emit: Emit) -> Result<Self> {
        let settings = store
            .setting::<Settings>(SETTINGS)?
            .unwrap_or_else(default_settings);
        let app = Self {
            store: Mutex::new(store),
            settings: Mutex::new(settings),
            review: Mutex::new(None),
            import: Mutex::new(None),
            renderers: Mutex::new(HashMap::new()),
            pages: Mutex::new(HashMap::new()),
            calls: Mutex::new(HashMap::new()),
            review_generation: AtomicU64::new(0),
            feed: Mutex::new(FeedRun::default()),
            feed_lane: tokio::sync::Mutex::new(()),
            emit,
        };
        let _ = export::import_applications_csv(&app);
        Ok(app)
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    /// Saves `s` and makes it current.
    pub fn put_settings(&self, s: &Settings) -> Result<()> {
        lock(&self.store).put_setting(SETTINGS, s)?;
        *lock(&self.settings) = s.clone();
        Ok(())
    }

    /// Runs `f` with the warm renderer of `master`'s template, making it on first use.
    pub fn with_renderer<T>(
        &self,
        master: &Master,
        f: impl FnOnce(&mut Renderer) -> Result<T>,
    ) -> Result<T> {
        let mut pool = lock(&self.renderers);
        let renderer = match pool.entry(master.template.clone()) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => e.insert(Renderer::new(
                &resume_pipeline::template(master)?,
                FontOptions::default(),
            )),
        };
        f(renderer)
    }

    /// Loads the default template's renderer (fonts, template parse) so the first review doesn't wait for it.
    pub fn warm(&self) {
        let master = Master {
            id: String::new(),
            title: String::new(),
            template: DEFAULT_TEMPLATE.to_owned(),
            page_limit: 1,
            sections: vec![],
        };
        let _ = self.with_renderer(&master, |_| Ok(()));
    }

    /// `master` resolved against `bank` and rendered once per revision pair: the score's inputs for its page now (page
    /// count and PDF read-back, as `resume_tailor` scores it).
    pub fn master_page(
        &self,
        bank_rev: &Rev,
        bank: &Bank,
        master_rev: &Rev,
        master: &Master,
    ) -> Result<MasterPage> {
        let key = (bank_rev.clone(), master_rev.clone());
        if let Some(p) = lock(&self.pages).get(&key) {
            return Ok(p.clone());
        }
        let resume = resolve(bank, master)?;
        let json = serde_json::to_string(&resume)?;
        let (pages, ats_safe, pdf_text) = self.with_renderer(master, |r| {
            let rendered = r.render(&json, false)?;
            let text = r.pdf_text(&rendered)?;
            Ok((rendered.metrics.pages, r.manifest().ats_safe, text))
        })?;
        let page = MasterPage {
            resume,
            facts: PageFacts {
                pages,
                page_limit: master.page_limit,
                ats_safe,
                pdf_text: Some(pdf_text),
            },
        };
        lock(&self.pages).insert(key, page.clone());
        Ok(page)
    }

    /// The current bank and every master, each with its page now. Empty without a bank.
    pub fn masters_now(&self) -> Result<Option<Library>> {
        let (bank, masters) = {
            let store = lock(&self.store);
            let Some(bank) = store.bank()? else {
                return Ok(None);
            };
            (bank, store.masters()?)
        };
        let masters = masters
            .into_iter()
            .map(|(rev, m)| {
                let page = self.master_page(&bank.0, &bank.1, &rev, &m)?;
                Ok((rev, m, page))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Some(Library { bank, masters }))
    }

    /// The `claude` the settings name, else the one found.
    pub async fn claude(&self) -> Result<ClaudeCli> {
        let path = self.settings().claude_path;
        match path.filter(|p| !p.trim().is_empty()) {
            Some(p) => Ok(ClaudeCli::new(p)),
            None => Ok(ClaudeCli::discover().await?),
        }
    }

    /// Stops the model call running for `job`, if any.
    pub fn cancel(&self, job: &str) {
        if let Some(h) = lock(&self.calls).remove(job) {
            h.abort();
        }
    }

    /// A ticket for a `start_review` call: newer calls get higher ones.
    pub fn review_ticket(&self) -> u64 {
        self.review_generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// `ticket` is the newest `start_review` call's.
    pub fn is_newest_review(&self, ticket: u64) -> bool {
        self.review_generation.load(Ordering::SeqCst) == ticket
    }
}

/// The bank and masters as of now.
pub struct Library {
    pub bank: (Rev, Bank),
    pub masters: Vec<(Rev, Master, MasterPage)>,
}

/// Runs `call` (a model call) as a task registered under `job`, after telling the UI it started: `cancel(job)` aborts
/// it, which drops the `claude` child and kills it. A newer call for the same job supersedes a running one.
pub async fn model_call<T: Send + 'static>(
    app: &Arc<App>,
    job: &str,
    stage: Stage,
    call: impl Future<Output = std::result::Result<T, LlmError>> + Send + 'static,
) -> Result<T> {
    (app.emit)(Event::Progress(&Progress {
        job: job.to_owned(),
        stage,
    }));
    let task = tokio::spawn(call);
    let handle = task.abort_handle();
    if let Some(old) = lock(&app.calls).insert(job.to_owned(), handle.clone()) {
        old.abort();
    }
    let result = task.await;
    {
        let mut calls = lock(&app.calls);
        if calls.get(job).is_some_and(|h| h.id() == handle.id()) {
            calls.remove(job);
        }
    }
    Ok(result??)
}

/// Runs `f` on the blocking pool: compiles and the store's heavier reads never hold up the async runtime.
pub async fn blocking<T: Send + 'static>(
    app: &Arc<App>,
    f: impl FnOnce(&App) -> Result<T> + Send + 'static,
) -> Result<T> {
    let app = Arc::clone(app);
    tokio::task::spawn_blocking(move || f(&app)).await?
}

fn default_settings() -> Settings {
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
    Settings {
        theme: Theme::Light,
        backdrop: Backdrop::None,
        default_mode: Mode::MasterOnly,
        extract_model: DEFAULT_EXTRACT_MODEL.to_owned(),
        suggest_model: DEFAULT_SUGGEST_MODEL.to_owned(),
        export_dir: home.join("Documents/Resumes").display().to_string(),
        claude_path: None,
        onboarded: false,
    }
}

impl From<Mode> for resume_core::change::Mode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::MasterOnly => Self::MasterOnly,
            Mode::BankAware => Self::BankAware,
        }
    }
}

impl From<resume_core::change::Mode> for Mode {
    fn from(m: resume_core::change::Mode) -> Self {
        match m {
            resume_core::change::Mode::MasterOnly => Self::MasterOnly,
            resume_core::change::Mode::BankAware => Self::BankAware,
        }
    }
}
