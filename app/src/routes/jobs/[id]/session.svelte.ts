// One job's review session: opens the posting (Extract), starts the review (Suggest) and runs every review command.
// Commands run one at a time, in the order you asked for them, so a response can never land after a later one; each
// queued edit reads the review as it stands when its turn comes (two quick clicks on a row toggle it twice).

import {
  addEdit,
  appError,
  cancel,
  cover,
  decide,
  exportCover,
  exportPdf,
  onProgress,
  openJob,
  redo,
  removeEdit,
  rewrite,
  saveVariant,
  rewriteCoverParagraph,
  setCoverPart,
  setWording,
  startReview,
  toggleSkill,
  undo,
  writeCover,
  type AppError,
  type ChangeState,
  type ChangeView,
  type CoverPart,
  type CoverView,
  type CustomEdit,
  type Decision,
  type Exported,
  type JobView,
  type Mode,
  type ReviewView,
  type Rewritten,
  type Stage,
} from "$lib/api";

export type Phase = "opening" | "suggesting" | "ready" | "stopped" | "failed";

export interface Running {
  stage: Stage;
  /** `performance.now()` when the event arrived. */
  startedAt: number;
}

/** A model call of this run that finished: Extract, before Suggest runs. */
export interface Step {
  stage: Stage;
  ms: number;
}

/** What a click on a row's checkbox asks for, from the state it shows. */
function nextDecision(state: ChangeState): Decision | null {
  switch (state) {
    case "in":
      return "off";
    case "off":
      return null;
    case "left-out":
      return "on";
    case "forced":
      return "off";
  }
}

export class Session {
  readonly job: string;
  /** The posting with its requirements, once Extract ran. */
  view = $state<JobView | null>(null);
  review = $state<ReviewView | null>(null);
  phase = $state<Phase>("opening");
  /** Why the review couldn't open (phase `failed`). */
  error = $state<AppError | null>(null);
  /** The model call running for this job, from its `progress` event; null when none (or a cached answer). */
  running = $state<Running | null>(null);
  /** The model calls of this run already done (Extract while Suggest runs), for the status's trail. */
  steps = $state<Step[]>([]);
  /** The change being rewritten from a note. */
  rewriting = $state<string | null>(null);
  /** What the toolbar shows: the review's master and mode, or the ones just asked for. */
  master = $state<string | null>(null);
  mode = $state<Mode | null>(null);
  /** Commands waiting or running. */
  pending = $state(0);
  /** Bumped when a review arrives from Suggest (not from an edit): the column enters once per arrival. */
  arrival = $state(0);
  /** The open review's cover letter, once loaded for it. */
  cover = $state<CoverView | null>(null);
  /** A model call on the letter: the whole letter, or paragraph `n` (from 0). */
  drafting = $state<"letter" | number | null>(null);

  /** Reports a command's failure (the screen toasts it). */
  onerror: (e: AppError) => void = () => {};

  #chain: Promise<unknown> = Promise.resolve();
  #alive = true;
  #unlisten: (() => void) | null = null;

  constructor(job: string) {
    this.job = job;
  }

  /** Listens for progress, then opens the posting and starts its review. */
  async start() {
    const unlisten = await onProgress((p) => {
      if (p.job === this.job && this.#alive)
        this.running = { stage: p.stage, startedAt: performance.now() };
    });
    if (!this.#alive) return unlisten();
    this.#unlisten = unlisten;
    await this.open();
  }

  /** Stops listening; a model call still running for this job is cancelled. */
  dispose() {
    this.#alive = false;
    this.#unlisten?.();
    if (this.running) void cancel(this.job).catch(() => {});
  }

  /** Extract (cached per posting), then Suggest with the job's last master and mode. */
  async open() {
    this.phase = "opening";
    this.error = null;
    this.steps = [];
    try {
      const view = await openJob(this.job);
      if (!this.#alive) return;
      this.view = view;
      this.master ??= view.masters[0]?.id ?? null;
    } catch (e) {
      this.running = null;
      return this.#stop(appError(e));
    }
    const done = this.running;
    if (done) this.steps = [{ stage: done.stage, ms: performance.now() - done.startedAt }];
    this.running = null;
    await this.suggest(null, null);
  }

  /** Opens the review for `master` × `mode` (null: the last ones); `fresh` asks Suggest again. */
  suggest(master: string | null, mode: Mode | null, fresh = false) {
    return this.#queue(async () => {
      if (!this.#alive) return;
      // A run of its own (another master, a re-run) starts its trail afresh; after `open` it carries Extract on.
      if (this.phase !== "opening") this.steps = [];
      this.phase = "suggesting";
      this.error = null;
      try {
        const review = await startReview(this.job, master, mode, fresh);
        if (!this.#alive) return;
        this.#show(review);
        this.arrival++;
        this.phase = "ready";
      } catch (e) {
        if (this.#alive) this.#stop(appError(e));
      } finally {
        this.running = null;
      }
    });
  }

  /** Stops the model call running for this job; its command then fails quietly as `cancelled`. */
  cancel() {
    void cancel(this.job).catch((e) => this.onerror(appError(e)));
  }

  toggle(key: string) {
    return this.#edit((id, review) => {
      const change = find(review, key);
      return change ? decide(id, key, nextDecision(change.state)) : null;
    });
  }

  word(key: string, text: string | null) {
    return this.#edit((id) => setWording(id, key, text));
  }

  /** Your own edit removed, or a suggestion back to its suggested wording. */
  remove(change: ChangeView) {
    return this.#edit((id) => (change.custom ? removeEdit(id, change.key) : setWording(id, change.key, null)));
  }

  /** The change's line saved to the bank as a variant of its bullet; true when it was. */
  saveVariant(key: string) {
    return this.#edit((id, review) => (find(review, key)?.savable ? saveVariant(id, key) : null));
  }

  skill(term: string) {
    return this.#edit((id) => toggleSkill(id, term));
  }

  add(edit: CustomEdit) {
    return this.#edit((id) => addEdit(id, edit));
  }

  undo() {
    return this.#edit((id, review) => (review.canUndo ? undo(id) : null));
  }

  redo() {
    return this.#edit((id, review) => (review.canRedo ? redo(id) : null));
  }

  /** One model call; resolves with the validator's flags (null when it didn't run). */
  rewrite(key: string, note: string): Promise<Rewritten | null> {
    return this.#queue(async () => {
      const review = this.review;
      if (!review || !this.#alive) return null;
      this.rewriting = key;
      try {
        const done = await rewrite(review.id, key, note);
        if (this.#alive) this.#show(done.review);
        return done;
      } catch (e) {
        if (this.#alive) this.#report(appError(e));
        return null;
      } finally {
        this.rewriting = null;
        this.running = null;
      }
    });
  }

  export(): Promise<Exported | null> {
    return this.#queue(async () => {
      const review = this.review;
      if (!review || !this.#alive) return null;
      try {
        return await exportPdf(review.id);
      } catch (e) {
        if (this.#alive) this.#report(appError(e));
        return null;
      }
    });
  }

  // ── Cover letter ──

  /** Loads the open review's letter (and its page). */
  loadCover() {
    return this.#letter((id) => cover(id));
  }

  /** One model call: the whole letter, written (again). */
  writeCover() {
    return this.#letter((id) => writeCover(id), "letter");
  }

  /** One part of the letter in your own words. */
  setCoverPart(part: CoverPart, text: string) {
    return this.#letter((id) => setCoverPart(id, part, text));
  }

  /** One model call: paragraph `index` rewritten per `note` (may be empty). */
  rewriteParagraph(index: number, note: string) {
    return this.#letter((id) => rewriteCoverParagraph(id, index, note), index);
  }

  exportCover(): Promise<Exported | null> {
    return this.#queue(async () => {
      const review = this.review;
      if (!review || !this.#alive) return null;
      try {
        return await exportCover(review.id);
      } catch (e) {
        if (this.#alive) this.#report(appError(e));
        return null;
      }
    });
  }

  /** A letter command for the open review; `drafting` marks a model call while it runs. Resolves true when it landed. */
  #letter(call: (id: string) => Promise<CoverView>, drafting: "letter" | number | null = null) {
    return this.#queue(async () => {
      const review = this.review;
      if (!review || !this.#alive || this.phase !== "ready") return false;
      this.drafting = drafting;
      try {
        const next = await call(review.id);
        if (!this.#alive || this.review?.id !== next.review) return false;
        this.cover = next;
        return true;
      } catch (e) {
        if (this.#alive) this.#report(appError(e));
        return false;
      } finally {
        this.drafting = null;
        if (drafting !== null) this.running = null;
      }
    });
  }

  /** Runs `task` after every command asked for before it; a failure doesn't stop the ones after it. */
  #queue<T>(task: () => Promise<T>): Promise<T> {
    this.pending++;
    const run = this.#chain.then(task);
    this.#chain = run.then(
      () => this.pending--,
      () => this.pending--,
    );
    return run;
  }

  /** A review command over the review as it stands when its turn comes; `call` returns null to skip. */
  #edit(call: (id: string, review: ReviewView) => Promise<ReviewView> | null) {
    return this.#queue(async () => {
      const review = this.review;
      if (!review || !this.#alive || this.phase !== "ready") return false;
      const pending = call(review.id, review);
      if (!pending) return false;
      try {
        const next = await pending;
        if (this.#alive) this.#show(next);
        return true;
      } catch (e) {
        if (this.#alive) this.#report(appError(e));
        return false;
      }
    });
  }

  #show(review: ReviewView) {
    this.review = review;
    this.master = review.master.id;
    this.mode = review.mode;
  }

  /** A failed open or Suggest: cancelling goes back quietly to what was there; anything else says why. */
  #stop(error: AppError) {
    this.master = this.review?.master.id ?? this.view?.masters[0]?.id ?? null;
    this.mode = this.review?.mode ?? null;
    if (this.review) {
      this.phase = "ready";
      if (error.kind !== "cancelled") this.onerror(error);
      return;
    }
    this.phase = error.kind === "cancelled" ? "stopped" : "failed";
    this.error = error.kind === "cancelled" ? null : error;
  }

  #report(error: AppError) {
    if (error.kind !== "cancelled") this.onerror(error);
  }
}

export function find(review: ReviewView, key: string): ChangeView | undefined {
  for (const group of review.groups) {
    const change = group.changes.find((c) => c.key === key);
    if (change) return change;
  }
}
