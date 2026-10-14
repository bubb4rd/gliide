//! What crosses the bridge: every command's arguments and results, and the events the UI listens to.
//!
//! These are view models, not the core's types: the UI gets lines ready to show (a change's text with the terms to
//! mark, its state and reason), never an `Op` to interpret. `cargo test -p gliide` regenerates their TypeScript
//! in `app/src/lib/api/bindings/` (ts-rs); the UI imports only those.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Errors ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// Every command's error: a sentence the UI shows as is (DESIGN.md Voice), and what kind of failure it was.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ErrorKind {
    /// You cancelled it.
    Cancelled,
    /// `claude` missing, signed out, timed out, or its output failed the schema.
    Agent,
    /// The posting couldn't be read (blocked site, empty page…).
    Ingest,
    /// No bank or master imported yet, or a stale id.
    Missing,
    /// SQLite, files.
    Storage,
    Other,
}

// ── Settings and the agent ──────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Theme {
    #[default]
    Light,
    Dark,
    System,
}

/// `master-only` or `bank-aware`, as `resume_core::change::Mode` serializes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Mode {
    #[default]
    MasterOnly,
    BankAware,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    pub theme: Theme,
    pub default_mode: Mode,
    /// A `claude --model` alias or name; `sonnet` by default.
    pub extract_model: String,
    pub suggest_model: String,
    /// Absolute folder for exported PDFs and `applications.csv`; `~/Documents/Resumes` by default.
    pub export_dir: String,
    /// The `claude` binary; `None` finds it (`resume_llm::find_claude`).
    pub claude_path: Option<String>,
}

/// What Settings and the sidebar show about `claude`: found (`claude --version`) and signed in
/// (`claude auth status`'s `loggedIn`). The app reads no credential.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentStatus {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    /// `None` when it couldn't be checked.
    pub signed_in: Option<bool>,
    /// `claude.ai` login, `pro`/`max` plan…, for the badge: `"Claude Code · Pro"`.
    pub plan: Option<String>,
    /// Why it isn't usable, and what to do: "Run `claude` in Terminal and sign in".
    pub problem: Option<String>,
}

// ── Bank and masters ────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Library {
    /// `None` before the first import.
    pub bank: Option<BankSummary>,
    pub masters: Vec<MasterRow>,
    /// The bank's sections in order, each entry with the masters that show it.
    pub sections: Vec<SectionRow>,
    pub templates: Vec<TemplateRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BankSummary {
    /// The person's name on the bank.
    pub name: String,
    /// Content hash of the current revision (12 hex).
    pub rev: String,
    pub entries: u32,
    pub bullets: u32,
    /// RFC 3339.
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MasterRow {
    pub id: String,
    /// The track: `Software Engineering`.
    pub title: String,
    pub template: String,
    pub page_limit: u32,
    pub entries: u32,
    /// Pages its resolved resume takes in its template.
    pub pages: u32,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SectionRow {
    pub id: String,
    pub title: String,
    /// `experience`, `projects`, `education`, `skills`, `summary`, `certifications` or `custom`.
    pub kind: String,
    pub entries: Vec<EntryRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EntryRow {
    pub id: String,
    /// The entry's name, or a skills line's label.
    pub name: String,
    pub subtitle: String,
    pub dates: String,
    /// Bullets, or a skills line's items.
    pub items: u32,
    pub tags: Vec<String>,
    /// Ids of the masters that show it.
    pub masters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TemplateRow {
    pub id: String,
    pub name: String,
    pub ats_safe: bool,
}

/// A resume file read for import, before anything is added: where each part goes, and the master it makes.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportView {
    /// The waiting import: `import_view` and `finish_import` name it.
    pub id: String,
    /// The file's name.
    pub file: String,
    /// The person's name as the file heads it.
    pub name: String,
    /// No bank yet: everything is new, and the file's name and contact line become the bank's.
    pub first: bool,
    pub sections: Vec<ImportSection>,
    /// The file's contact items the bank lacks; adding one puts it on every resume.
    pub contact: Vec<ImportContact>,
    /// Dates that can’t be read: "Can’t read the dates “Summer 2023” of …".
    pub warnings: Vec<String>,
    /// Entries added (kept-separate ones included).
    pub new_entries: u32,
    /// Entries merged into the bank's same entry.
    pub same_entries: u32,
    pub new_bullets: u32,
    pub new_items: u32,
    /// The master it makes, new lines highlighted, one SVG document per page.
    pub pages: Vec<String>,
    pub page_count: u32,
    pub page_limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportSection {
    /// The heading as the file prints it (the new master shows it).
    pub title: String,
    /// The bank section it joins, by its title; `None` for a new section.
    pub into: Option<String>,
    pub entries: Vec<ImportEntry>,
    pub groups: Vec<ImportGroup>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportEntry {
    /// What `ImportChoices.separate` names.
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub dates: String,
    pub bullets: u32,
    /// The bank entry with the same name and first month: merged into it unless kept separate.
    pub same: Option<SameEntry>,
    /// Kept separate: added as an entry of its own.
    pub separate: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SameEntry {
    pub name: String,
    pub subtitle: String,
    pub dates: String,
    /// The file's bullets the entry has already (as a bullet or a variant).
    pub matched: u32,
    /// The file's bullets merging adds to it.
    pub new: u32,
    /// `subtitle`, `place`, `dates`, `link`, `notes`: where the file prints something else; merging keeps the bank's.
    pub differs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportGroup {
    pub label: String,
    pub items: u32,
    /// The bank skills line it joins, by its label; `None` for a new line.
    pub into: Option<String>,
    /// Items that line lacks (all of them for a new line).
    pub new_items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportContact {
    pub text: String,
    pub url: String,
    pub chosen: bool,
}

/// How to add a waiting import.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportChoices {
    /// `ImportView.id`.
    pub import: String,
    /// Ids of entries with a same entry to add as entries of their own.
    pub separate: Vec<String>,
    /// Indexes into `ImportView.contact` to add to the contact line.
    pub contact: Vec<u32>,
    /// A master the import replaces (keeping its title, template and page limit); `None` makes a new master.
    pub replace: Option<String>,
    /// The new master's title.
    pub title: String,
}

/// One bank entry in its editor: every bullet with its wordings (its text, then its variants) and which master shows
/// which, and the desk's preview.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EntryView {
    pub id: String,
    /// The bank section's title.
    pub section: String,
    pub name: String,
    pub subtitle: String,
    pub place: String,
    pub dates: String,
    /// Note lines as printed: `Label: text`, or the text alone.
    pub notes: Vec<String>,
    pub bullets: Vec<BulletView>,
    /// The masters that show this entry, by title.
    pub masters: Vec<MasterRef>,
    /// The master on the desk; `None`: the entry alone, every bullet in its text.
    pub preview: Option<String>,
    /// The preview, the entry's bullets highlighted, one SVG document per page.
    pub pages: Vec<String>,
    pub page_count: u32,
    pub page_limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MasterRef {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulletView {
    pub id: String,
    /// The bullet's text first (`variant` null), then each variant.
    pub wordings: Vec<WordingView>,
    /// Skills you say it demonstrates.
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WordingView {
    /// `None` for the bullet's text, else the variant's index.
    pub variant: Option<u32>,
    pub text: String,
    /// Ids of the masters that show this wording.
    pub masters: Vec<String>,
}

/// An edit of a bank entry's bullets and their wordings. Every master keeps showing the words it showed, except where
/// the edit says otherwise (`show`, or editing the wording it shows).
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[ts(export)]
pub enum EntryEdit {
    /// A new variant of a bullet.
    AddVariant {
        bullet: String,
        text: String,
    },
    /// New words for one wording (`variant` null: the bullet's text), in every master that shows it.
    SetWording {
        bullet: String,
        variant: Option<u32>,
        text: String,
    },
    RemoveVariant {
        bullet: String,
        variant: u32,
    },
    /// A variant becomes the bullet's text, and the text a variant.
    MakeMain {
        bullet: String,
        variant: u32,
    },
    /// Bullet `from` becomes variants of `into`, a bullet of the same entry.
    Fold {
        from: String,
        into: String,
    },
    /// Which wording `master` shows.
    Show {
        master: String,
        bullet: String,
        variant: Option<u32>,
    },
}

// ── Jobs ────────────────────────────────────────────────────────────────────────────────────────────────────────

/// A posting in Applications (as "Not sent" until it has an application) and the sidebar's Recent.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobRow {
    /// The posting's id.
    pub id: String,
    /// From Extract once it ran; else the posting's own metadata; else `"Untitled posting"`.
    pub role: String,
    pub company: String,
    /// The company's logo key (`LogoRow.key`); empty without a company name.
    pub company_key: String,
    pub location: String,
    pub url: Option<String>,
    /// The latest review's projected score, else the best master's score now; `None` before Extract.
    pub score: Option<f64>,
    pub status: Option<AppStatus>,
    pub added_at: String,
    /// Yours once you've set or removed it, else the posting's (its source's fields, else its text).
    pub salary: Option<SalaryRange>,
}

/// A salary range: either end open ("up to"), in one currency, per year or per hour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SalaryRange {
    /// In cents (minor units); `None` for "up to".
    pub min_cents: Option<u32>,
    /// In cents; `None` for "from". Equal to `minCents` for one amount.
    pub max_cents: Option<u32>,
    /// ISO 4217: `USD`.
    pub currency: String,
    pub period: SalaryPeriod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum SalaryPeriod {
    Year,
    Hour,
}

/// A posting with its requirements and every master scored against it: what opens before Suggest runs.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobView {
    pub job: JobRow,
    pub head: JobHead,
    /// Best first (`resume_core::rank`); the first is preselected.
    pub masters: Vec<MasterChoice>,
    /// Requirement skills in the posting's order (no have/add state yet).
    pub skills: Vec<SkillChip>,
    pub knockouts: Vec<String>,
}

/// The job header (DESIGN.md Review): badge, meta line, serif headline.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobHead {
    pub role: String,
    pub company: String,
    /// The company's logo key (`LogoRow.key`); empty without a company name.
    pub company_key: String,
    pub location: String,
    pub seniority: String,
    pub url: Option<String>,
    /// `"read from globex.com"`, `"from Greenhouse"`, `"pasted"`.
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MasterChoice {
    pub id: String,
    pub title: String,
    /// Its score now against this posting.
    pub score: f64,
}

// ── Review ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// The open review: everything the column and the desk draw. Every review command returns a fresh one.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewView {
    /// The analysis id: review commands name it, so a stale screen can't edit another review.
    pub id: String,
    pub job: JobRow,
    pub head: JobHead,
    pub verdict: Verdict,
    /// Suggest's one line.
    pub headline: String,
    pub master: MasterChoice,
    pub masters: Vec<MasterChoice>,
    pub mode: Mode,
    pub now: ScoreView,
    pub projected: ScoreView,
    pub ceiling: ScoreView,
    pub skills: Vec<SkillChip>,
    /// In the resume's section order, then `bank` ("From your bank") last; empty groups omitted.
    pub groups: Vec<ChangeGroup>,
    /// The master's lines no change touches, in page order ("Other lines"): Edit and Rewrite with a note make one
    /// your edit (`add_edit` with `CustomEdit::Line`/`Summary`, `rewrite_line`).
    pub lines: Vec<LineView>,
    /// Requirements the resume can't back, each with one honest fix.
    pub gaps: Vec<GapView>,
    pub knockouts: Vec<String>,
    /// The tailored resume, highlighted, one SVG document per page.
    pub pages: Vec<String>,
    pub page_count: u32,
    pub page_limit: u32,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Wall time of the last tailor step (fit, scores, points, SVG), ms.
    pub tailor_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Verdict {
    Apply,
    Tailor,
    Stretch,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScoreView {
    /// 0–100, one decimal.
    pub total: f64,
    pub components: Vec<ComponentView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComponentView {
    /// `hard_tool`, `domain_soft`, `title`, `experience`, `education`, `format`.
    pub id: String,
    /// `Hard skills and tools`, `Domain and soft`, `Job title`, `Experience`, `Education`, `Format`.
    pub label: String,
    pub weight: f64,
    /// The posting specifies it; absent components don't count.
    pub present: bool,
    /// 0–1.
    pub value: f64,
    /// The gist in a few words: `2 of 3 · missing Docker`, `Partly: Software, Engineering`.
    pub brief: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ChipState {
    /// On the page as the master has it.
    Have,
    /// On the page because a change that is in adds it.
    Add,
    /// Not on the page.
    Missing,
    /// You said it isn't yours: removed from the page, never suggested again for this review.
    NotMine,
}

/// A requirement skill. Clicking toggles it (`toggle_skill`): `missing`/`not-mine` → on the page (a change adding it
/// to a skills line), `have`/`add` → `not-mine` (the adding change off, or a skill removal of yours).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkillChip {
    pub term: String,
    /// 1 most important … 3. Priority-1 chips are weight 600 (`chip--p1`).
    pub priority: u8,
    pub required: bool,
    pub state: ChipState,
    /// Why a click can't remove it (`"Stated in a bullet: edit the bullet"`); `None` when it can toggle.
    pub locked: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangeGroup {
    /// A section id, or `bank`.
    pub id: String,
    /// `Work · Chamber of Commerce`-style label: the section title, sentence case; `From your bank`.
    pub label: String,
    /// Sum of the points of the changes that are in.
    pub points: f64,
    pub changes: Vec<ChangeView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ChangeState {
    In,
    Off,
    LeftOut,
    Forced,
}

/// One change row (DESIGN.md `.change`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangeView {
    pub key: String,
    /// The op: `rewrite`, `use_variant`, `cut`, `add`, `skill_add`, `skill_remove`, `skill_rename`, `skill_move`,
    /// `note_add`, `swap`, `set`, `include`, `exclude`, `swap_entry`, `reorder`, `summary`.
    pub op: String,
    /// What the row reads: the line as it will read for line edits; a sentence otherwise
    /// (`Add CI/CD to Tools & Technologies`, `Include Campus IT Help Desk, Arizona State University, 2023–2024`).
    pub text: String,
    /// Terms inside `text` to mark as inserted (`.mark`).
    pub marks: Vec<String>,
    /// The line it replaces, for line edits (`Was “…”`).
    pub was: Option<String>,
    /// Meta under the row: `Rewrite · same 2 lines`, `Add skill · adds SQL` (terms the text doesn't show marked).
    pub meta: String,
    pub state: ChangeState,
    /// Left out: why (`"40%" isn't in your resume`, `Takes 3 lines, not 2`).
    pub reason: Option<String>,
    /// Kept although it runs long (yours and forced).
    pub warning: Option<String>,
    /// Marginal points (see `resume_tailor` docs); `None` when it can't be toggled alone.
    pub points: Option<f64>,
    /// You wrote it (`Your edit`): an own change, or a suggestion whose wording you changed.
    pub yours: bool,
    /// One of your own edits (`remove_edit` takes it off); `false` for a suggestion, reworded or not
    /// (`set_wording(null)` restores its wording).
    pub custom: bool,
    /// Its wording can be edited inline (line edits and summary); the field starts from `text`.
    pub editable: bool,
    /// Rewrite with a note applies (bullet and summary edits).
    pub rewritable: bool,
    /// Its line can be saved to the bank as a variant of its bullet (`save_variant`): a bullet edit that isn't left
    /// out and reads unlike every wording the bullet has.
    pub savable: bool,
    /// Suggest's reason, for the hover/selected detail.
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GapView {
    pub skill: String,
    pub fix: String,
}

/// Your decision on a change: `on`, `off`, or `null` for the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Decision {
    On,
    Off,
}

/// An edit of your own (`a` in the review).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[ts(export)]
pub enum CustomEdit {
    /// A new bullet at the end of an entry.
    Bullet { entry: String, text: String },
    /// A skill added to a skills line.
    Skill { group: String, item: String },
    /// A bullet or note line in your own words.
    Line { line: String, text: String },
    /// The summary line.
    Summary { text: String },
}

/// Where your own new edits can go: the open review's entries and skills lines, for the `a` picker. Existing lines
/// are edited in place (`ReviewView::lines`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditTargets {
    pub entries: Vec<Target>,
    pub groups: Vec<Target>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Target {
    pub id: String,
    /// What the picker shows: an entry's `Name · subtitle`, a skills line's label.
    pub label: String,
}

/// A line on the master that no change touches, as "Other lines" shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LineView {
    /// The bullet or note line (`entry#n`) it is, for `CustomEdit::Line`; `None` for the summary
    /// (`CustomEdit::Summary`).
    pub line: Option<String>,
    /// Where it sits: its entry's `Name · subtitle` (and a note line's label), or `Summary`.
    pub place: String,
    pub text: String,
    /// Rewrite with a note applies: bullets and the summary, not note lines.
    pub rewritable: bool,
}

/// What `rewrite` did: the review with the new wording in, and the grounding validator's findings on it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Rewritten {
    pub review: ReviewView,
    pub flags: Vec<String>,
}

// ── Export and applications ─────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Exported {
    /// Absolute path of the PDF.
    pub path: String,
    /// `globex_jr_sde_doe_resume.pdf`.
    pub file_name: String,
    /// The score from the PDF's own text ("on file"); `None` when the text couldn't be read back.
    pub on_file: Option<f64>,
    pub application: ApplicationRow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AppStatus {
    Saved,
    Applied,
    Interviewing,
    Offer,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApplicationRow {
    pub id: String,
    /// The posting, when the application came from one in the app (not from an imported ATS Fit CSV row).
    pub job: Option<String>,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub company: String,
    /// The company's logo key (`LogoRow.key`); empty without a company name.
    pub company_key: String,
    pub role: String,
    pub now: Option<f64>,
    pub projected: Option<f64>,
    pub on_file: Option<f64>,
    pub resume: Option<String>,
    pub cover_letter: Option<String>,
    pub url: Option<String>,
    pub status: AppStatus,
}

// ── The export folder ───────────────────────────────────────────────────────────────────────────────────────────

/// The export folder's PDFs, newest first: what Resumes lists beside the masters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderView {
    /// Absolute path of the export folder.
    pub dir: String,
    /// Empty when the folder holds no PDF yet, or doesn't exist yet.
    pub files: Vec<FolderFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderFile {
    /// `globex_jr_sde_doe_resume.pdf`: what `folder_pdf` takes.
    pub name: String,
    /// Absolute path.
    pub path: String,
    /// Last modified, RFC 3339.
    pub modified_at: String,
    pub kind: FileKind,
}

/// What a PDF in the export folder is, from its name: the app's exports end in `_resume` or `_cover_letter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum FileKind {
    /// The name says cover letter (`cover_letter`, `Cover Letter`, `coverletter`…).
    CoverLetter,
    /// Otherwise, the name says resume or CV.
    Resume,
    /// Neither.
    Other,
}

/// One of the export folder's PDFs, for the desk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderPdf {
    pub name: String,
    pub path: String,
    /// One SVG document per page.
    pub pages: Vec<String>,
}

// ── Company logos ───────────────────────────────────────────────────────────────────────────────────────────────

/// A company with a logo, yours or fetched: the UI shows it from `logo://localhost/<key>?v=<version>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LogoRow {
    /// The company's name lowercased, each run of anything but letters and digits as one `_`: postings and
    /// applications with the same name share the logo.
    pub key: String,
    /// When the logo was set: a new logo gets a new URL, so no stale image shows.
    pub version: String,
}

/// Where a logo you set comes from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[ts(export)]
pub enum LogoSource {
    /// An image file: chosen, or dropped on the mark.
    File { path: String },
    /// The clipboard: a copied image file, else the copied picture.
    Clipboard,
}

// ── Cover letter ────────────────────────────────────────────────────────────────────────────────────────────────

/// The open review's cover letter: its parts and its page, rendered under the resume's letterhead.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoverView {
    /// The review (analysis) the letter belongs to.
    pub review: String,
    /// `None` until it's written.
    pub letter: Option<Letter>,
    /// One SVG document per page; empty without a letter.
    pub pages: Vec<String>,
    pub page_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Letter {
    pub greeting: String,
    /// 3 or 4 as written; yours to edit.
    pub paragraphs: Vec<String>,
    pub closing: String,
    /// Words across the paragraphs (the prompt asks for 250–350).
    pub words: u32,
}

/// One part of the letter, for `set_cover_part`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[ts(export)]
pub enum CoverPart {
    Greeting,
    /// Paragraph `index`, from 0.
    Paragraph {
        index: u32,
    },
    Closing,
}

// ── Job feed ────────────────────────────────────────────────────────────────────────────────────────────────────

/// Where a feed job stands. A checked job is a fit when its best master's score now reaches the threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum FeedItemState {
    /// Pulled, waiting for its check.
    New,
    /// Left out by the local check (`reason` says why), or its posting couldn't be read.
    Skipped,
    /// Over the day's checks: checked after the next pull.
    Waiting,
    /// Its check is running.
    Checking,
    Fit,
    NotFit,
    Dismissed,
}

/// A job from one of the feed's sources: a feed row, or a search result.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FeedItem {
    /// The feed's id for the job.
    pub slug: String,
    /// The source that found it, as a person reads it (`freehire`, `LinkedIn`, `Watchlist`, `Fantastic.jobs`).
    pub origin: String,
    pub role: String,
    pub company: String,
    /// The company's logo key (`LogoRow.key`), as `JobRow.companyKey`.
    pub company_key: String,
    pub location: String,
    /// The employer's link (LinkedIn's job page for a LinkedIn job).
    pub url: String,
    pub state: FeedItemState,
    /// The best master's score now, once checked.
    pub score: Option<f64>,
    /// The best master's title, once checked.
    pub master: Option<String>,
    /// The posting's id once read: what `/jobs/{id}` opens.
    pub job: Option<String>,
    /// The employer's posting date (RFC 3339), when the source states one.
    pub posted_at: Option<String>,
    /// When its source first recorded it (RFC 3339).
    pub found_at: String,
    /// Its requirements' knockouts, once checked.
    pub knockouts: Vec<String>,
    /// Why it was skipped, or why its check failed.
    pub reason: Option<String>,
    pub salary: Option<SalaryRange>,
}

/// Event `feed`, and the feed's heading: the pull and the day's checks.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FeedStatus {
    /// Pulls run on their own (Settings → Feed).
    pub enabled: bool,
    /// LinkedIn is pulled and searched beside freehire (Settings → Feed).
    pub linkedin: bool,
    /// The sources are being read.
    pub pulling: bool,
    /// The slug of the job being checked.
    pub checking: Option<String>,
    /// The last pull that finished (RFC 3339).
    pub last_pull: Option<String>,
    /// When the next pull runs (RFC 3339); `None` with the feed off.
    pub next_pull: Option<String>,
    /// Checks since local midnight.
    pub checked_today: u32,
    pub daily_checks: u32,
    /// The fit threshold, in percent.
    pub threshold: f64,
    /// A source the feed is leaving alone for now, and until when (LinkedIn after it limited requests); not an error.
    pub paused: Option<String>,
    /// Why the last pull or check failed.
    pub error: Option<String>,
}

/// The feed: every row but skipped and dismissed ones. Fits first (best score first), then not fits, the one being
/// checked, waiting, new.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FeedView {
    pub items: Vec<FeedItem>,
    pub status: FeedStatus,
}

/// One page of a search: freehire's matches in its order, then LinkedIn's. Jobs the feed has show as stored, the
/// others as `new`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchResults {
    pub items: Vec<FeedItem>,
    /// Every match freehire has, plus LinkedIn's found so far (LinkedIn doesn't say how many it has).
    #[ts(type = "number")]
    pub total: u64,
    /// Where the next page starts; `None`: no more from either source.
    pub next: Option<SearchCursor>,
    /// A source that failed on this page, as a sentence (`LinkedIn: …`); the other's results still show.
    pub error: Option<String>,
}

/// Where a search's next page starts in each source.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchCursor {
    /// freehire's offset; `None`: freehire has no more.
    pub freehire: Option<u32>,
    /// LinkedIn's `start`; `None`: LinkedIn is off, failed or has no more.
    pub linkedin: Option<u32>,
    /// LinkedIn's found so far, for `total`.
    pub linkedin_found: u32,
    /// freehire's matches, for `total` once freehire has no more.
    #[ts(type = "number")]
    pub freehire_total: u64,
}

/// Settings → Feed: the settings, what Auto works out now, and the masters to choose from.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FeedSettingsView {
    pub settings: FeedSettingsDto,
    pub auto: FeedAuto,
    pub masters: Vec<MasterRef>,
    /// The watchlist's boards, in its order.
    pub boards: Vec<WatchBoard>,
}

/// A company board on the watchlist.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchBoard {
    /// The board's careers link, as stored.
    pub url: String,
    /// The ATS (`Greenhouse`, `Workday`).
    pub ats: String,
    /// The board's company slug or tenant (`wing`, `nvidia`).
    pub name: String,
}

/// The feed's settings. `None` on a filter is Auto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FeedSettingsDto {
    /// Pulls run on their own: at launch and every `everyHours`.
    pub enabled: bool,
    /// The master the feed looks for; `None`: every master.
    pub master: Option<String>,
    pub seniority: Option<Vec<String>>,
    pub employment: Option<Vec<String>>,
    pub category: Option<Vec<String>>,
    /// Lowercase ISO codes (`us`).
    pub countries: Option<Vec<String>>,
    pub needs_visa: bool,
    pub include_clearance: bool,
    /// Jobs freehire couldn't place.
    pub include_unknown_location: bool,
    pub daily_checks: u32,
    /// In percent.
    pub threshold: f64,
    pub every_hours: u32,
    /// Search LinkedIn on each pull.
    pub linkedin: bool,
    /// Company careers links polled on each pull (Greenhouse, Lever, Ashby, Workday, SmartRecruiters, Workable,
    /// Recruitee).
    pub watchlist: Vec<String>,
    /// Apify API token for Fantastic.jobs; `None` leaves it off.
    pub apify_token: Option<String>,
}

/// What Auto works out from the bank and the chosen master (empty without a bank).
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FeedAuto {
    /// Years of experience, overlaps merged.
    pub years: f64,
    pub seniority: Vec<String>,
    pub employment: Vec<String>,
    pub category: Vec<String>,
    pub countries: Vec<String>,
}

// ── Events ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// Event `progress`: a model call started for a job. The UI shows it until the command returns.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Progress {
    /// The posting id the step runs for (what `cancel` takes).
    pub job: String,
    pub stage: Stage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Stage {
    Extracting,
    Suggesting,
    Rewriting,
    /// The cover letter, written whole.
    Writing,
    /// One paragraph of the cover letter, rewritten with a note.
    Revising,
    /// A resume file copied for import.
    Importing,
}
