// The app's commands, typed. Every call goes through here; screens never call `invoke` themselves.
// Types come from `bindings/` (generated from app/src-tauri/src/dto.rs by `cargo test -p easyshmeezy`).
// A failed command rejects with an `AppError` ({ kind, message }); show `message` as is.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { AgentStatus } from "./bindings/AgentStatus";
import type { AppError } from "./bindings/AppError";
import type { ApplicationRow } from "./bindings/ApplicationRow";
import type { AppStatus } from "./bindings/AppStatus";
import type { CustomEdit } from "./bindings/CustomEdit";
import type { Decision } from "./bindings/Decision";
import type { EditTargets } from "./bindings/EditTargets";
import type { Exported } from "./bindings/Exported";
import type { JobRow } from "./bindings/JobRow";
import type { JobView } from "./bindings/JobView";
import type { Library } from "./bindings/Library";
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
export type * from "./bindings/ChangeGroup";
export type * from "./bindings/ChangeState";
export type * from "./bindings/ChangeView";
export type * from "./bindings/ChipState";
export type * from "./bindings/ComponentView";
export type * from "./bindings/CustomEdit";
export type * from "./bindings/Decision";
export type * from "./bindings/EditTargets";
export type * from "./bindings/EntryRow";
export type * from "./bindings/ErrorKind";
export type * from "./bindings/Exported";
export type * from "./bindings/GapView";
export type * from "./bindings/JobHead";
export type * from "./bindings/JobRow";
export type * from "./bindings/JobView";
export type * from "./bindings/Library";
export type * from "./bindings/MasterChoice";
export type * from "./bindings/MasterRow";
export type * from "./bindings/Mode";
export type * from "./bindings/Progress";
export type * from "./bindings/ReviewView";
export type * from "./bindings/Rewritten";
export type * from "./bindings/ScoreView";
export type * from "./bindings/SectionRow";
export type * from "./bindings/Settings";
export type * from "./bindings/SkillChip";
export type * from "./bindings/Stage";
export type * from "./bindings/Target";
export type * from "./bindings/TemplateRow";
export type * from "./bindings/Theme";
export type * from "./bindings/Verdict";

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
/** Imports an ATS Fit `master.json` (v1): a new bank revision and its `swe` master. */
export const importResume = (path: string) => invoke<Library>("import_resume", { path });
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
/** The PDF (no highlights) into the export folder, its application row, and the score read back from it. */
export const exportPdf = (review: string) => invoke<Exported>("export_pdf", { review });

// ── Applications ──
export const applications = () => invoke<ApplicationRow[]>("applications");
export const setStatus = (application: string, status: AppStatus) =>
  invoke<ApplicationRow>("set_status", { application, status });

// ── Events ──
export const onProgress = (handler: (p: Progress) => void): Promise<UnlistenFn> =>
  listen<Progress>("progress", (e) => handler(e.payload));
