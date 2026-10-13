// The app's commands, typed. Every call goes through here; screens never call `invoke` themselves.
// Types come from `bindings/` (generated from app/src-tauri/src/dto.rs by `cargo test -p gliide`).
// A failed command rejects with an `AppError` ({ kind, message }); show `message` as is.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { AgentStatus } from "./bindings/AgentStatus";
import type { AppError } from "./bindings/AppError";
import type { ApplicationRow } from "./bindings/ApplicationRow";
import type { AppStatus } from "./bindings/AppStatus";
import type { CoverPart } from "./bindings/CoverPart";
import type { CoverView } from "./bindings/CoverView";
import type { CustomEdit } from "./bindings/CustomEdit";
import type { Decision } from "./bindings/Decision";
import type { EditTargets } from "./bindings/EditTargets";
import type { EntryEdit } from "./bindings/EntryEdit";
import type { EntryView } from "./bindings/EntryView";
import type { Exported } from "./bindings/Exported";
import type { ImportChoices } from "./bindings/ImportChoices";
import type { ImportView } from "./bindings/ImportView";
import type { JobRow } from "./bindings/JobRow";
import type { JobView } from "./bindings/JobView";
import type { Library } from "./bindings/Library";
import type { LogoRow } from "./bindings/LogoRow";
import type { LogoSource } from "./bindings/LogoSource";
import type { Mode } from "./bindings/Mode";
import type { Progress } from "./bindings/Progress";
import type { ReviewView } from "./bindings/ReviewView";
import type { Rewritten } from "./bindings/Rewritten";
import type { Settings } from "./bindings/Settings";

export type * from "./bindings/AgentStatus";
export type * from "./bindings/AppError";
export type * from "./bindings/ApplicationRow";
export type * from "./bindings/AppStatus";
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
export type * from "./bindings/EditTargets";
export type * from "./bindings/EntryEdit";
export type * from "./bindings/EntryRow";
export type * from "./bindings/EntryView";
export type * from "./bindings/ErrorKind";
export type * from "./bindings/Exported";
export type * from "./bindings/GapView";
export type * from "./bindings/ImportChoices";
export type * from "./bindings/ImportContact";
export type * from "./bindings/ImportEntry";
export type * from "./bindings/ImportGroup";
export type * from "./bindings/ImportSection";
export type * from "./bindings/ImportView";
export type * from "./bindings/JobHead";
export type * from "./bindings/JobRow";
export type * from "./bindings/JobView";
export type * from "./bindings/Letter";
export type * from "./bindings/Library";
export type * from "./bindings/LineView";
export type * from "./bindings/LogoRow";
export type * from "./bindings/LogoSource";
export type * from "./bindings/MasterChoice";
export type * from "./bindings/MasterRef";
export type * from "./bindings/MasterRow";
export type * from "./bindings/Mode";
export type * from "./bindings/Progress";
export type * from "./bindings/ReviewView";
export type * from "./bindings/Rewritten";
export type * from "./bindings/SameEntry";
export type * from "./bindings/ScoreView";
export type * from "./bindings/SectionRow";
export type * from "./bindings/Settings";
export type * from "./bindings/SkillChip";
export type * from "./bindings/Stage";
export type * from "./bindings/Target";
export type * from "./bindings/TemplateRow";
export type * from "./bindings/Theme";
export type * from "./bindings/Verdict";
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

// ── Bank and masters ──
export const library = () => invoke<Library>("library");
/** `cancel` and `progress` name the import's model call with this job key. */
export const IMPORT_JOB = "import";
/**
 * Reads a resume file (.pdf, .docx, .md, .txt: one model call, emits `progress` for `IMPORT_JOB`; ATS Fit's
 * master.json: none) and holds it for review: where each part goes, and the new master it makes, rendered.
 */
export const readImport = (path: string) => invoke<ImportView>("read_import", { path });
/** The waiting import as `choices` would add it. */
export const importView = (choices: ImportChoices) => invoke<ImportView>("import_view", { choices });
/** Adds the waiting import to the bank and saves its master. */
export const finishImport = (choices: ImportChoices) => invoke<Library>("finish_import", { choices });
export const discardImport = () => invoke<void>("discard_import");
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

// ── Jobs ──
export const jobs = () => invoke<JobRow[]>("jobs");
/** Reads a link or pasted text into a posting (no model call). */
export const addJob = (input: string) => invoke<JobRow>("add_job", { input });
/** The posting's requirements (Extract, cached per posting) and every master scored. Emits `progress`. */
export const openJob = (job: string) => invoke<JobView>("open_job", { job });
export const deleteJob = (job: string) => invoke<JobRow[]>("delete_job", { job });
/**
 * Opens the review for `job`: Suggest (cached per posting × master × mode × model), then the fit check.
 * `master`/`mode` null: the job's last review's, else the best-ranked master and the default mode.
 * `fresh` calls Suggest again (`r`). Emits `progress`.
 */
export const startReview = (job: string, master: string | null, mode: Mode | null, fresh = false) =>
  invoke<ReviewView>("start_review", { job, master, mode, fresh });
/** Stops the model call running for `job`; its command rejects with kind `cancelled`. */
export const cancel = (job: string) => invoke<void>("cancel", { job });

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
export const setStatus = (application: string, status: AppStatus) =>
  invoke<ApplicationRow>("set_status", { application, status });

// ── Company logos ──
export const logos = () => invoke<LogoRow[]>("logos");
/** Sets the company's logo from an image file or the clipboard (a copied image file, else the copied picture), replacing
 * any. Rejects with a sentence to show when it isn't an image. */
export const setLogo = (company: string, source: LogoSource) => invoke<LogoRow>("set_logo", { company, source });
export const removeLogo = (company: string) => invoke<void>("remove_logo", { company });
/** The URL a logo shows from; a new logo gets a new one. */
export const logoUrl = (logo: LogoRow) => `${convertFileSrc(logo.key, "logo")}?v=${encodeURIComponent(logo.version)}`;

// ── Events ──
export const onProgress = (handler: (p: Progress) => void): Promise<UnlistenFn> =>
  listen<Progress>("progress", (e) => handler(e.payload));
