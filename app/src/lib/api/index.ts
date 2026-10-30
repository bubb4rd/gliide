// The app's commands, typed. Every call goes through here; screens never call `invoke` themselves.
// Types come from `bindings/` (generated from app/src-tauri/src/dto.rs by `cargo test -p gliide`).
// A failed command rejects with an `AppError` ({ kind, message }); show `message` as is.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Accepted } from "./bindings/Accepted";
import type { AgentStatus } from "./bindings/AgentStatus";
import type { AppError } from "./bindings/AppError";
import type { ApplicationRow } from "./bindings/ApplicationRow";
import type { AppStatus } from "./bindings/AppStatus";
import type { CoverPart } from "./bindings/CoverPart";
import type { CoverView } from "./bindings/CoverView";
import type { CustomEdit } from "./bindings/CustomEdit";
import type { Debrief } from "./bindings/Debrief";
import type { DebriefView } from "./bindings/DebriefView";
import type { Decision } from "./bindings/Decision";
import type { DigAnswer } from "./bindings/DigAnswer";
import type { DigCapture } from "./bindings/DigCapture";
import type { DigQuestions } from "./bindings/DigQuestions";
import type { EditTargets } from "./bindings/EditTargets";
import type { EntryEdit } from "./bindings/EntryEdit";
import type { EntryView } from "./bindings/EntryView";
import type { Exported } from "./bindings/Exported";
import type { FeedSettingsDto } from "./bindings/FeedSettingsDto";
import type { FeedSettingsView } from "./bindings/FeedSettingsView";
import type { FeedStatus } from "./bindings/FeedStatus";
import type { FeedView } from "./bindings/FeedView";
import type { FolderPdf } from "./bindings/FolderPdf";
import type { FolderView } from "./bindings/FolderView";
import type { ImportChoices } from "./bindings/ImportChoices";
import type { ImportView } from "./bindings/ImportView";
import type { Instructed } from "./bindings/Instructed";
import type { JobRow } from "./bindings/JobRow";
import type { JobView } from "./bindings/JobView";
import type { KeyDecision } from "./bindings/KeyDecision";
import type { Library } from "./bindings/Library";
import type { LogoRow } from "./bindings/LogoRow";
import type { LogoSource } from "./bindings/LogoSource";
import type { NotificationPermission } from "./bindings/NotificationPermission";
import type { Nudge } from "./bindings/Nudge";
import type { Place } from "./bindings/Place";
import type { PracticeTurn } from "./bindings/PracticeTurn";
import type { PrepView } from "./bindings/PrepView";
import type { Progress } from "./bindings/Progress";
import type { ReviewView } from "./bindings/ReviewView";
import type { Rewritten } from "./bindings/Rewritten";
import type { Round } from "./bindings/Round";
import type { RoundDraft } from "./bindings/RoundDraft";
import type { RoundsView } from "./bindings/RoundsView";
import type { SalaryRange } from "./bindings/SalaryRange";
import type { SearchCursor } from "./bindings/SearchCursor";
import type { SearchResults } from "./bindings/SearchResults";
import type { SentView } from "./bindings/SentView";
import type { Settings } from "./bindings/Settings";
import type { StartupProblem } from "./bindings/StartupProblem";
import type { StatusSet } from "./bindings/StatusSet";
import type { StoriesView } from "./bindings/StoriesView";
import type { StoryDraft } from "./bindings/StoryDraft";
import type { StoryView } from "./bindings/StoryView";
import type { UpdateStatus } from "./bindings/UpdateStatus";

export type * from "./bindings/Accepted";
export type * from "./bindings/AgentStatus";
export type * from "./bindings/AppError";
export type * from "./bindings/ApplicationRow";
export type * from "./bindings/AppStatus";
export type * from "./bindings/Backdrop";
export type * from "./bindings/BankSummary";
export type * from "./bindings/BulletView";
export type * from "./bindings/ChangeGroup";
export type * from "./bindings/ChangeState";
export type * from "./bindings/ChangeView";
export type * from "./bindings/ChipState";
export type * from "./bindings/ComponentView";
export type * from "./bindings/CoverPart";
export type * from "./bindings/CoverView";
export type * from "./bindings/CustomEdit";
export type * from "./bindings/Decision";
export type * from "./bindings/Displaced";
export type * from "./bindings/Asked";
export type * from "./bindings/Debrief";
export type * from "./bindings/DebriefDig";
export type * from "./bindings/DebriefRead";
export type * from "./bindings/DebriefView";
export type * from "./bindings/DigAnswer";
export type * from "./bindings/DigCapture";
export type * from "./bindings/DigOp";
export type * from "./bindings/DigQuestion";
export type * from "./bindings/DigQuestions";
export type * from "./bindings/DigSkip";
export type * from "./bindings/EditTargets";
export type * from "./bindings/EntryDetails";
export type * from "./bindings/EntryEdit";
export type * from "./bindings/EntryRow";
export type * from "./bindings/EntryView";
export type * from "./bindings/ErrorKind";
export type * from "./bindings/Exported";
export type * from "./bindings/FactKind";
export type * from "./bindings/FactLine";
export type * from "./bindings/Feedback";
export type * from "./bindings/FeedAuto";
export type * from "./bindings/FeedItem";
export type * from "./bindings/FeedItemState";
export type * from "./bindings/FeedSettingsDto";
export type * from "./bindings/FeedSettingsView";
export type * from "./bindings/FeedStatus";
export type * from "./bindings/FeedView";
export type * from "./bindings/FileKind";
export type * from "./bindings/FolderFile";
export type * from "./bindings/FolderPdf";
export type * from "./bindings/FolderView";
export type * from "./bindings/GapView";
export type * from "./bindings/ImportChoices";
export type * from "./bindings/ImportContact";
export type * from "./bindings/ImportEntry";
export type * from "./bindings/ImportGroup";
export type * from "./bindings/ImportSection";
export type * from "./bindings/ImportView";
export type * from "./bindings/Instructed";
export type * from "./bindings/JobHead";
export type * from "./bindings/JobRow";
export type * from "./bindings/JobView";
export type * from "./bindings/KeyDecision";
export type * from "./bindings/Letter";
export type * from "./bindings/Library";
export type * from "./bindings/LineView";
export type * from "./bindings/LogoRow";
export type * from "./bindings/LogoSource";
export type * from "./bindings/MasterChoice";
export type * from "./bindings/MasterRef";
export type * from "./bindings/MasterRow";
export type * from "./bindings/Mismatch";
export type * from "./bindings/Mode";
export type * from "./bindings/NewClaim";
export type * from "./bindings/NoteLine";
export type * from "./bindings/NotificationPermission";
export type * from "./bindings/Notifications";
export type * from "./bindings/Nudge";
export type * from "./bindings/Place";
export type * from "./bindings/PracticeTurn";
export type * from "./bindings/PrepGap";
export type * from "./bindings/PrepQuestion";
export type * from "./bindings/PrepView";
export type * from "./bindings/Proposal";
export type * from "./bindings/Progress";
export type * from "./bindings/QuestionGroup";
export type * from "./bindings/ReviewView";
export type * from "./bindings/Rewritten";
export type * from "./bindings/Round";
export type * from "./bindings/RoundDraft";
export type * from "./bindings/RoundKind";
export type * from "./bindings/RoundState";
export type * from "./bindings/RoundsView";
export type * from "./bindings/SalaryPeriod";
export type * from "./bindings/SalaryRange";
export type * from "./bindings/SameEntry";
export type * from "./bindings/ScoreView";
export type * from "./bindings/SearchCursor";
export type * from "./bindings/SearchResults";
export type * from "./bindings/SectionRow";
export type * from "./bindings/SentFrom";
export type * from "./bindings/SentLine";
export type * from "./bindings/SentView";
export type * from "./bindings/Settings";
export type * from "./bindings/SkillChip";
export type * from "./bindings/Skipped";
export type * from "./bindings/Stage";
export type * from "./bindings/StarPart";
export type * from "./bindings/StartupKind";
export type * from "./bindings/StartupProblem";
export type * from "./bindings/StatusSet";
export type * from "./bindings/StoriesView";
export type * from "./bindings/StoryDraft";
export type * from "./bindings/StoryLink";
export type * from "./bindings/StorySource";
export type * from "./bindings/StoryTheme";
export type * from "./bindings/StoryUse";
export type * from "./bindings/StoryView";
export type * from "./bindings/Target";
export type * from "./bindings/TemplateRow";
export type * from "./bindings/Theme";
export type * from "./bindings/UpdateState";
export type * from "./bindings/UpdateStatus";
export type * from "./bindings/Verdict";
export type * from "./bindings/WatchBoard";
export type * from "./bindings/WordingView";

/** `e` as an `AppError`: commands reject with one; anything else (a bridge failure) becomes `other`. */
export function appError(e: unknown): AppError {
  if (e && typeof e === "object" && "message" in e && "kind" in e) return e as AppError;
  return { kind: "other", message: e instanceof Error ? e.message : String(e) };
}

// ── Settings and the agent ──
export const agentStatus = () => invoke<AgentStatus>("agent_status");
export const getSettings = () => invoke<Settings>("get_settings");
export const setSettings = (settings: Settings) => invoke<Settings>("set_settings", { settings });
/** Each asked `claude --model` alias or name → the model id that last served it (what names a chat's model). */
export const models = () => invoke<Record<string, string>>("models");

// ── Startup ──
/** Why the data couldn't be opened at launch (the window then shows only Startup); `null` when it opened. */
export const startupProblem = () => invoke<StartupProblem | null>("startup_problem");
/** Quits and launches gliide again. */
export const relaunch = () => invoke<void>("relaunch");

// ── Updates ──
export const updateStatus = () => invoke<UpdateStatus>("update_status");
export const checkUpdate = () => invoke<UpdateStatus>("check_update");
/** Downloads and installs the release the last check found, then relaunches; rejects only when that failed. */
export const installUpdate = () => invoke<void>("install_update");

// ── Bank and masters ──
export const library = () => invoke<Library>("library");
/** `cancel` and `progress` name the import's model call with this job key. */
export const IMPORT_JOB = "import";
/**
 * Reads a resume file (.pdf, .docx, .md, .txt: one model call, emits `progress` for `IMPORT_JOB`; a v1 resume
 * JSON: none) and holds it for review: where each part goes, and the new master it makes, rendered.
 */
export const readImport = (path: string) => invoke<ImportView>("read_import", { path });
/** The waiting import as `choices` would add it. */
export const importView = (choices: ImportChoices) => invoke<ImportView>("import_view", { choices });
/** Adds the waiting import to the bank and saves its master. */
export const finishImport = (choices: ImportChoices) => invoke<Library>("finish_import", { choices });
export const discardImport = () => invoke<void>("discard_import");
/** Adds an entry named `name` at the end of bank section `section`, in no master: its id. Rejects with a sentence to
 * show (no name, or a section that holds no entries). */
export const addEntry = (section: string, name: string) => invoke<string>("add_entry", { section, name });
/** One bank entry with every bullet's wordings and which master shows which; `preview` picks the master on the desk
 * (null: the first that shows the entry; the entry alone when none does). */
export const entry = (entry: string, preview: string | null = null) =>
  invoke<EntryView>("entry", { entry, preview });
/** Edits an entry's bullet wordings (a new bank revision; masters keep the words they showed unless the edit is about
 * what one shows). Rejects with a sentence to show, e.g. when a master shows both bullets of a fold. */
export const editEntry = (entry: string, edit: EntryEdit, preview: string | null = null) =>
  invoke<EntryView>("edit_entry", { entry, edit, preview });
export const setMasterOptions = (master: string, template: string, pageLimit: number) =>
  invoke<Library>("set_master_options", { master, template, pageLimit });

// ── The export folder ──
/** The export folder's PDFs, newest first. */
export const folder = () => invoke<FolderView>("folder");
/** One of them (`FolderFile.name`), its pages as SVG. */
export const folderPdf = (name: string) => invoke<FolderPdf>("folder_pdf", { name });

// ── Jobs ──
export const jobs = () => invoke<JobRow[]>("jobs");
/** Reads a link or pasted text into a posting (no model call). */
export const addJob = (input: string) => invoke<JobRow>("add_job", { input });
/** The posting's requirements (Extract, cached per posting) and every master scored. Emits `progress`. */
export const openJob = (job: string) => invoke<JobView>("open_job", { job });
export const deleteJob = (job: string) => invoke<JobRow[]>("delete_job", { job });
/** Sets the posting's salary range as yours (`null`: removed); its row with the range it now shows. */
export const setSalary = (job: string, salary: SalaryRange | null) => invoke<JobRow>("set_salary", { job, salary });
/** The places a job's location names, geocoded and cached (empty: remote, a country, or nothing found). */
export const places = (location: string) => invoke<Place[]>("places", { location });
/**
 * Opens the review for `job`: Suggest (cached per posting × master × Settings' suggestions-from × model; a call
 * already running for the same joins it), then the fit check. `master` null: the job's last review's, else the
 * best-ranked master. `fresh` calls Suggest again (`r`). Emits `progress`.
 */
export const startReview = (job: string, master: string | null, fresh = false) =>
  invoke<ReviewView>("start_review", { job, master, fresh });
/** As `startReview` with the job's last master, saved (with its fit score) without opening it. */
export const prepareReview = (job: string) => invoke<void>("prepare_review", { job });
/** Stops the model call running for `job`; its command rejects with kind `cancelled`. */
export const cancel = (job: string) => invoke<void>("cancel", { job });
/** The model calls running, oldest first. */
export const running = () => invoke<Progress[]>("running");
/** When a running call started, on the `performance.now()` clock. */
export const startedAt = (p: Progress) => performance.now() - (Date.now() - p.startedAt);

// ── Review (each returns the whole review, rebuilt) ──
export const decide = (review: string, key: string, decision: Decision | null) =>
  invoke<ReviewView>("decide", { review, key, decision });
/** Your wording for a change; null restores the suggested wording. */
export const setWording = (review: string, key: string, text: string | null) =>
  invoke<ReviewView>("set_wording", { review, key, text });
export const addEdit = (review: string, edit: CustomEdit) => invoke<ReviewView>("add_edit", { review, edit });
export const removeEdit = (review: string, key: string) => invoke<ReviewView>("remove_edit", { review, key });
export const toggleSkill = (review: string, term: string) => invoke<ReviewView>("toggle_skill", { review, term });
export const undo = (review: string) => invoke<ReviewView>("undo", { review });
export const redo = (review: string) => invoke<ReviewView>("redo", { review });
export const editTargets = (review: string) => invoke<EditTargets>("edit_targets", { review });
/** One model call: the line rewritten per your note, set as your wording. Emits `progress`. */
export const rewrite = (review: string, key: string, note: string) =>
  invoke<Rewritten>("rewrite", { review, key, note });
/**
 * One model call: a line no change touches (`LineView.line`; null for the summary) rewritten per your note, added as
 * your edit. Emits `progress`.
 */
export const rewriteLine = (review: string, line: string | null, note: string) =>
  invoke<Rewritten>("rewrite_line", { review, line, note });
/** Saves the change's line to the bank as a variant of its bullet (`ChangeView.savable`); the review goes on as it is. */
export const saveVariant = (review: string, key: string) => invoke<ReviewView>("save_variant", { review, key });
/** Keeps "I have `skill` experience" as a fact of bank entry `entry`: no review asks about it there again. */
export const keepFact = (review: string, entry: string, skill: string) =>
  invoke<ReviewView>("keep_fact", { review, entry, skill });
/**
 * One model call: `instruction` carried out over the page (or only `scope`: entry, bullet or skills-line ids, or
 * `"summary"`), each edit added as your own and off: pending until `accept`. One undo step. Emits `progress`.
 * `confirmed` names skills you say you have though the resume doesn't show them (a skipped part's `term`).
 */
export const instruct = (review: string, instruction: string, scope: string[], confirmed: string[] = []) =>
  invoke<Instructed>("instruct", { review, instruction, scope, confirmed });
/** Turns these changes on, one undo step; any other change in on the same line goes off (`displaced`). */
export const accept = (review: string, keys: string[]) => invoke<Accepted>("accept", { review, keys });
/** One undo step: your own edits among `keys` removed, suggestions turned off. */
export const reject = (review: string, keys: string[]) => invoke<ReviewView>("reject", { review, keys });
/**
 * A rewind of the chat, one undo step: `keys` off the page (your own edits removed, suggestions turned off), `restore`
 * (changes accepting them turned off) back to their default.
 */
export const withdraw = (review: string, keys: string[], restore: string[]) =>
  invoke<ReviewView>("withdraw", { review, keys, restore });
/** Several decisions as one undo step (`decision` null: back to the change's default). */
export const settle = (review: string, decisions: KeyDecision[]) => invoke<ReviewView>("settle", { review, decisions });
/** The PDF (no highlights) into the export folder, its application row, and the score read back from it. */
export const exportPdf = (review: string) => invoke<Exported>("export_pdf", { review });

// ── Cover letter (the open review's; each returns the letter, rendered) ──
/** The letter and its page; `letter` is null until it's written. */
export const cover = (review: string) => invoke<CoverView>("cover", { review });
/** One model call: the whole letter from the tailored resume as the draft has it now and the posting. Emits `progress`. */
export const writeCover = (review: string) => invoke<CoverView>("write_cover", { review });
/** One part of the letter in your own words. */
export const setCoverPart = (review: string, part: CoverPart, text: string) =>
  invoke<CoverView>("set_cover_part", { review, part, text });
/** One model call: paragraph `index` (from 0) rewritten per your note (may be empty). Emits `progress`. */
export const rewriteCoverParagraph = (review: string, index: number, note: string) =>
  invoke<CoverView>("rewrite_cover_paragraph", { review, index, note });
/** The letter's PDF into the export folder, recorded on the posting's application. Refuses over one page. */
export const exportCover = (review: string) => invoke<Exported>("export_cover", { review });

// ── Applications ──
export const applications = () => invoke<ApplicationRow[]>("applications");
/** Sets each application's status (all or none), rewriting applications.csv once. Moving to Offer or Rejected cancels
 *  their upcoming rounds (`cancelled`; `restoreRounds` takes them back). */
export const setStatus = (applications: string[], status: AppStatus) =>
  invoke<StatusSet>("set_status", { applications, status });

// ── Interviews ──
/** An application's rounds: scheduled ones by when they start, then the unscheduled. */
export const rounds = (application: string) => invoke<Round[]>("rounds", { application });
/** Saves a round (a new one when `draft.id` is null). The first upcoming round of a Saved or Applied application
 *  moves it to Interviewing (`movedFrom` says from what). */
export const saveRound = (application: string, draft: RoundDraft) =>
  invoke<RoundsView>("save_round", { application, draft });
export const removeRound = (round: string) => invoke<RoundsView>("remove_round", { round });
/** Rounds a status change cancelled, upcoming again. */
export const restoreRounds = (rounds: string[]) => invoke<ApplicationRow[]>("restore_rounds", { rounds });
/** One model call: a pasted invite read into a round's fields (each it could read). Emits `progress` (`inviting`). */
export const readInvite = (application: string, text: string) =>
  invoke<RoundDraft>("read_invite", { application, text });
/** A calendar invite file (`.ics`) read into a round's fields; no model call. */
export const readInviteFile = (path: string) => invoke<RoundDraft>("read_invite_file", { path });
/** The resume the application sent: what it exported, rendered again (`marked` lines highlighted), else the exported
 *  PDF. */
export const sentResume = (application: string, marked: string[] = []) =>
  invoke<SentView>("sent_resume", { application, marked });
/** A round's prep: kept, else written now (one model call; emits `progress`, `prepping`). `again` writes it anew. */
export const prep = (round: string, again = false) => invoke<PrepView>("prep", { round, again });
/** A round's practiced answers, oldest first. */
export const practiceTurns = (round: string) => invoke<PracticeTurn[]>("practice_turns", { round });
/** One model call: your answer to prep question `question` read (emits `progress`, `practicing`), kept with it. */
export const practice = (round: string, question: string, answer: string) =>
  invoke<PracticeTurn>("practice", { round, question, answer });
/** One model call (emits `progress`, `nudging`): nudges for prep question `question` while you answer it, read
 *  with `draft` (what you've typed; may be empty) and your last answer to it. Not kept. */
export const nudge = (round: string, question: string, draft: string) =>
  invoke<Nudge[]>("nudge", { round, question, draft });
/** Keeps what an answer said as a fact of bank entry `entry` (from practice for `round`; `null` for a story told
 *  alone, which was said for no posting). */
export const keepClaim = (round: string | null, entry: string, text: string) =>
  invoke<void>("keep_claim", { round, entry, text });
/** One model call (emits `progress`, `practicing`): `answer` to `asked` (a question the story answers, from
 *  `$lib/stories`), telling story `story` alone, read back and kept. */
export const tellStory = (story: string, asked: string, answer: string) =>
  invoke<PracticeTurn>("tell_story", { story, asked, answer });
/** The story's practiced tellings (Tell it), oldest first. */
export const storyTurns = (story: string) => invoke<PracticeTurn[]>("story_turns", { story });

// ── Stories ──
export const stories = () => invoke<StoriesView>("stories");
export const story = (id: string) => invoke<StoryView>("story", { id });
/** Saves a story (a new one when `draft.id` is null, or one Undo brings back by its id). Words drawn from an answer
 *  (`draft.from`) are checked against it; words you typed are yours. */
export const saveStory = (draft: StoryDraft) => invoke<StoryView>("save_story", { draft });
export const removeStory = (id: string) => invoke<void>("remove_story", { id });

// ── Dig: gliide asks about an entry ──
/** One model call (emits `progress`, `digging`, keyed by the entry): the entry read and its questions; kept until the
 *  entry changes, unless `again`. */
export const digQuestions = (entry: string, again = false) => invoke<DigQuestions>("dig_questions", { entry, again });
/** One model call: your answers read; the facts in them kept at once, additions proposed. */
export const digCapture = (entry: string, answers: DigAnswer[], note: string) =>
  invoke<DigCapture>("dig_capture", { entry, answers, note });

// ── Debriefs and notifications ──
/** A round's debrief, if it has one. */
export const debrief = (round: string) => invoke<DebriefView | null>("debrief", { round });
/** Saves a round's debrief as you wrote it (no model call); its `read` is kept until the debrief changes. */
export const saveDebrief = (round: string, debrief: Debrief) => invoke<DebriefView>("save_debrief", { round, debrief });
/** One model call: the saved debrief read (emits `progress`, `debriefing`); what it found is kept with it. */
export const readDebrief = (round: string) => invoke<DebriefView>("read_debrief", { round });
/** Whether macOS lets gliide show banners. */
export const notificationPermission = () => invoke<NotificationPermission>("notification_permission");
/** Asks macOS for permission (only Turn on calls it); what it now allows. */
export const requestNotifications = () => invoke<NotificationPermission>("request_notifications");
/** System Settings → Notifications. */
export const openNotificationSettings = () => invoke<void>("open_notification_settings");

// ── Job feed (freehire) ──
/** Every row but skipped and dismissed: fits first (best score first), then not fit, checking, waiting, new. */
export const feed = () => invoke<FeedView>("feed");
/** Pulls new postings from every source that's on and checks them up to the daily cap; answers once both are done. */
export const pullFeed = () => invoke<FeedView>("pull_feed");
/** freehire's search with the feed's filters plus `query`, then LinkedIn's when it's on (no model call); `cursor` from
 *  the last page's `next` for more. */
export const searchFeed = (query: string, cursor: SearchCursor | null = null) =>
  invoke<SearchResults>("search_feed", { query, cursor });
/** Adds these jobs to the feed if missing and checks them now, past the daily cap. */
export const checkFeed = (slugs: string[]) => invoke<FeedView>("check_feed", { slugs });
/** Takes the job off the feed (its posting too, when you never opened it). */
export const dismissFeed = (slug: string) => invoke<FeedView>("dismiss_feed", { slug });
/** Reads and stores the posting if needed and marks it opened: from then on it's in Recent and Applications. */
export const openFeedJob = (slug: string) => invoke<JobRow>("open_feed_job", { slug });
export const feedSettings = () => invoke<FeedSettingsView>("feed_settings");
export const setFeedSettings = (settings: FeedSettingsDto) =>
  invoke<FeedSettingsView>("set_feed_settings", { settings });

// ── Company logos ──
export const logos = () => invoke<LogoRow[]>("logos");
/** Sets the company's logo from an image file or the clipboard (a copied image file, else the copied picture), replacing
 * any. Rejects with a sentence to show when it isn't an image. */
export const setLogo = (company: string, source: LogoSource) => invoke<LogoRow>("set_logo", { company, source });
export const removeLogo = (company: string) => invoke<void>("remove_logo", { company });
/** The URL a logo shows from; a new logo gets a new one. */
export const logoUrl = (logo: LogoRow) => `${convertFileSrc(logo.key, "logo")}?v=${encodeURIComponent(logo.version)}`;

// ── Events ──
/** Event `progress`: the model calls running (as `running`), whenever one starts or ends. */
export const onProgress = (handler: (calls: Progress[]) => void): Promise<UnlistenFn> =>
  listen<Progress[]>("progress", (e) => handler(e.payload));
/** Event `feed`: a row or the feed's status changed. */
export const onFeed = (handler: (s: FeedStatus) => void): Promise<UnlistenFn> =>
  listen<FeedStatus>("feed", (e) => handler(e.payload));
/** Event `update`: checking, what a check found, or how an install is going. */
export const onUpdate = (handler: (s: UpdateStatus) => void): Promise<UnlistenFn> =>
  listen<UpdateStatus>("update", (e) => handler(e.payload));
/** Event `notification`: a round's macOS banner was clicked; the page it opens (`/jobs/<id>`, `?debrief=<round>`). */
export const onNotification = (handler: (path: string) => void): Promise<UnlistenFn> =>
  listen<string>("notification", (e) => handler(e.payload));
