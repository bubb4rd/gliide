// One job's review session: opens the posting (Extract), starts the review (Suggest) and runs every review command.
// Commands run one at a time, in the order you asked for them, so a response can never land after a later one; each
// queued edit reads the review as it stands when its turn comes (two quick clicks on a row toggle it twice).

import {
  accept,
  addEdit,
  appError,
  cancel,
  cover,
  decide,
  exportCover,
  exportPdf,
  instruct,
  onProgress,
  openJob,
  prepareReview,
  redo,
  reject,
  rewriteCoverParagraph,
  setCoverPart,
  setWording,
  settle,
  startedAt,
  startReview,
  undo,
  withdraw,
  writeCover,
  type AppError,
  type ChangeState,
  type ChangeView,
  type CoverPart,
  type CoverView,
  type Decision,
  type Exported,
  type JobView,
  type KeyDecision,
  type Mode,
  type ReviewView,
  type Stage,
} from "$lib/api";
import { app } from "$lib/stores/app.svelte";
import { loadThread, saveThread, type Mention, type ProposalState, type Turn, type TurnProposal } from "./thread";

export type Phase = "opening" | "suggesting" | "ready" | "stopped" | "failed";

/** The calls that go on when you leave the job: their answers are stored, so its review is ready when you're back. */
const KEEPS: Stage[] = ["extracting", "suggesting"];

/** Sessions whose page closed while your ask ran there, by job: kept until it's answered or the page opens again. */
const kept = new Map<string, Session>();

/** The session for `job`'s page: the kept one, while your ask there still runs, else a new one. */
export function sessionFor(job: string): Session {
  const s = kept.get(job);
  kept.delete(job);
  return s ?? new Session(job);
}

export interface Running {
  stage: Stage;
  /** When it started, on the `performance.now()` clock. */
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
  /** The model call running for this job (from `progress`, or left running when this opened); null when none. */
  running = $state<Running | null>(null);
  /** The model calls of this run already done (Extract while Suggest runs), for the status's trail. */
  steps = $state<Step[]>([]);
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
  /** What you asked the agent on this review, oldest first: the chat's turns. */
  turns = $state<Turn[]>([]);

  /** Reports a command's failure (the screen toasts it). */
  onerror: (e: AppError) => void = () => {};

  #chain: Promise<unknown> = Promise.resolve();
  #alive = true;
  #started = false;
  /** Its page closed while your ask ran: in `kept` until the ask is answered or the page opens again. */
  #kept = false;
  #unlisten: (() => void) | null = null;
  /** The review whose thread `turns` holds. */
  #thread: string | null = null;

  constructor(job: string) {
    this.job = job;
  }

  /**
   * Listens for progress, then opens the posting and starts its review (joining a call left running for it). A kept
   * session, its page open again, opens the review again behind your ask (it may not be the open one any more); the
   * thread stays as it is.
   */
  async start() {
    if (this.#started) {
      this.#kept = false;
      return this.suggest(this.master, this.mode);
    }
    this.#started = true;
    const unlisten = await onProgress((calls) => {
      const call = calls.find((c) => c.job === this.job);
      if (call && this.#alive) this.running = { stage: call.stage, startedAt: startedAt(call) };
    });
    if (!this.#alive) return unlisten();
    this.#unlisten = unlisten;
    const call = app.calls.find((c) => c.job === this.job);
    if (call) this.running = { stage: call.stage, startedAt: startedAt(call) };
    await this.open();
  }

  /**
   * Its page closed. With your ask still running, the session is kept (`sessionFor` gives it back) until it's
   * answered; otherwise it's closed.
   */
  leave() {
    this.onerror = () => {};
    if (this.instructing) {
      this.#kept = true;
      kept.set(this.job, this);
    } else this.#close();
  }

  /** Stops listening. Extract and Suggest go on (`KEEPS`); any other call for this job is cancelled. */
  #close() {
    this.#alive = false;
    this.#unlisten?.();
    if (this.running && !KEEPS.includes(this.running.stage)) void cancel(this.job).catch(() => {});
  }

  /** Extract (cached per posting), then Suggest with the job's last master and mode: both, even if you leave. */
  async open() {
    this.phase = "opening";
    this.error = null;
    this.steps = [];
    try {
      const view = await openJob(this.job);
      if (!this.#alive) {
        void prepareReview(this.job)
          .catch((e) => console.error(`prepare review: ${appError(e).message}`))
          .finally(() => app.refreshJobs());
        return;
      }
      this.view = view;
      this.master ??= view.masters[0]?.id ?? null;
    } catch (e) {
      this.running = null;
      return this.#stop(appError(e));
    }
    // Extract done joins the status's trail; a Suggest already running (left running for this job) stays the status.
    const done = this.running;
    if (done?.stage === "extracting") {
      this.steps = [{ stage: done.stage, ms: performance.now() - done.startedAt }];
      this.running = null;
    }
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
        // Suggest ran (not a cached answer): its alias may name a newer model now.
        if (this.running) void app.refreshModels();
        this.phase = "ready";
      } catch (e) {
        if (this.#alive) this.#stop(appError(e));
      } finally {
        this.running = null;
        // Left while it ran: the review was saved all the same, with the fit score Recent shows.
        if (!this.#alive) void app.refreshJobs();
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

  undo() {
    return this.#edit((id, review) => (review.canUndo ? undo(id) : null));
  }

  redo() {
    return this.#edit((id, review) => (review.canRedo ? redo(id) : null));
  }

  /** The instruction the agent is carrying out, if any. */
  get instructing(): Turn | null {
    return this.turns.find((t) => t.status === "running") ?? null;
  }

  /**
   * One model call: `ask` carried out over the page (or within `scope`), its edits added as yours and off, pending,
   * in a new turn; `confirmed` names skills you said you have though the resume doesn't show them. Resolves with the
   * turn (null when it couldn't start).
   */
  instruct(ask: string, scope: Mention[], confirmed: string[] = []): Promise<Turn | null> {
    return this.#queue(async () => {
      const review = this.review;
      if (!review || !this.#alive || this.phase !== "ready") return null;
      const thread = review.id;
      const list = this.turns;
      list.push({
        id: crypto.randomUUID(),
        at: Date.now(),
        ask,
        scope,
        ...(confirmed.length ? { confirmed } : {}),
        status: "running",
        reply: "",
        proposals: [],
        skipped: [],
        ms: 0,
        error: null,
      });
      const turn = list[list.length - 1];
      saveThread(this.job, thread, $state.snapshot(list));
      const started = performance.now();
      try {
        const done = await instruct(
          review.id,
          ask,
          scope.map((m) => m.id),
          confirmed,
        );
        turn.reply = done.reply;
        turn.model = done.model;
        void app.refreshModels();
        turn.proposals = done.proposals.map((p) => ({
          ...p,
          displaced: [],
          rejected: false,
        }));
        turn.skipped = done.skipped;
        turn.status = "done";
        if (this.#alive) this.#show(done.review);
      } catch (e) {
        const error = appError(e);
        turn.status = error.kind === "cancelled" ? "stopped" : "failed";
        turn.error = error.kind === "cancelled" ? null : error.message;
        if (this.#alive) this.#report(error);
      } finally {
        turn.ms = performance.now() - started;
        this.running = null;
        // The review may have changed under the call (another master): the turn stays with the one it was asked on.
        saveThread(this.job, thread, $state.snapshot(list));
        // Answered with its page closed: the session is done.
        if (this.#kept) {
          if (kept.get(this.job) === this) kept.delete(this.job);
          this.#kept = false;
          this.#close();
        }
      }
      return turn;
    });
  }

  /**
   * Your answer to whether you have a skill the agent left out (`turn.skipped[index].term`): yes asks the same again,
   * with that skill confirmed (resolves with the new turn); no leaves it out. Either way the question is answered.
   */
  answer(turn: Turn, index: number, yes: boolean): Promise<Turn | null> {
    const skip = turn.skipped[index];
    if (!skip?.term || skip.answer) return Promise.resolve(null);
    skip.answer = yes ? "yes" : "no";
    this.#saveThread();
    if (!yes) return Promise.resolve(null);
    const confirmed = [...(turn.confirmed ?? []), skip.term];
    return this.instruct(turn.ask, turn.scope, [...new Map(confirmed.map((t) => [t.toLowerCase(), t])).values()]);
  }

  /** What asking again from `from` takes back: the asks after it, and its and their edits accepted onto the page. */
  rewinding(from: Turn): { later: number; accepted: number } {
    const at = this.turns.findIndex((t) => t.id === from.id);
    const review = this.review;
    if (at < 0 || !review) return { later: 0, accepted: 0 };
    const gone = this.turns.slice(at);
    return {
      later: gone.length - 1,
      accepted: gone.flatMap((t) => t.proposals).filter((p) => proposalState(review, p) === "accepted").length,
    };
  }

  /**
   * Asks again from where `from` was, as `ask` within `scope` (its confirmed skills kept): `from` and every turn after
   * it leave the thread, and their edits leave the page in one undo step (the changes accepting them turned off back
   * on); then the ask goes out as a new turn. Resolves with it (null when it couldn't start).
   */
  async resend(from: Turn, ask: string, scope: Mention[]): Promise<Turn | null> {
    const at = () => this.turns.findIndex((t) => t.id === from.id);
    if (at() < 0 || this.instructing) return null;
    const gone = this.turns.slice(at()).flatMap((t) => t.proposals);
    const confirmed = from.confirmed ?? [];
    if (gone.length) {
      const done = await this.#edit((id, review) => {
        const keys = gone.map((p) => p.key).filter((k) => find(review, k));
        const restore = gone.flatMap((p) => p.displaced).filter((k) => find(review, k));
        return keys.length || restore.length ? withdraw(id, keys, restore) : null;
      });
      // Nothing of theirs on the page is fine; a failed withdraw keeps the thread as it was.
      if (!done && gone.some((p) => this.review && find(this.review, p.key))) return null;
    }
    const i = at();
    if (i < 0) return null;
    this.turns.splice(i);
    this.#saveThread();
    return this.instruct(ask, scope, confirmed);
  }

  /** Turns changes on (proposals or suggestions); one that takes another change's line turns that one off. */
  accept(keys: string[]) {
    return this.#queue(async () => {
      const review = this.review;
      if (!review || !this.#alive || this.phase !== "ready" || !keys.length) return false;
      try {
        const done = await accept(review.id, keys);
        if (!this.#alive) return false;
        this.#show(done.review);
        for (const p of this.#proposals(keys))
          p.displaced = done.displaced.filter((d) => d.by === p.key).map((d) => d.key);
        this.#saveThread();
        return true;
      } catch (e) {
        if (this.#alive) this.#report(appError(e));
        return false;
      }
    });
  }

  /** Your own edits among `keys` removed, suggestions turned off: one undo step. */
  async reject(keys: string[]) {
    if (!keys.length) return false;
    const done = await this.#edit((id) => reject(id, keys));
    if (done) {
      for (const p of this.#proposals(keys)) p.rejected = true;
      this.#saveThread();
    }
    return done;
  }

  /** An accepted proposal back to pending, and the changes it turned off back on. */
  async takeBack(p: TurnProposal) {
    const decisions: KeyDecision[] = [
      { key: p.key, decision: "off" },
      ...p.displaced.map((key) => ({ key, decision: null })),
    ];
    const done = await this.#edit((id) => settle(id, decisions));
    if (done) {
      p.displaced = [];
      this.#saveThread();
    }
    return done;
  }

  /** A rejected proposal added again, pending (two undo steps: the edit, then setting it aside). */
  restore(p: TurnProposal) {
    return this.#queue(async () => {
      const review = this.review;
      if (!review || !this.#alive || this.phase !== "ready") return false;
      try {
        const before = new Set(review.groups.flatMap((g) => g.changes.map((c) => c.key)));
        let next = await addEdit(review.id, p.edit);
        const added = next.groups.flatMap((g) => g.changes).find((c) => c.custom && !before.has(c.key));
        if (added) next = await settle(next.id, [{ key: added.key, decision: "off" }]);
        if (!this.#alive) return false;
        this.#show(next);
        if (added) {
          p.key = added.key;
          p.rejected = false;
          this.#saveThread();
        }
        return true;
      } catch (e) {
        if (this.#alive) this.#report(appError(e));
        return false;
      }
    });
  }

  /** The turns' proposals among `keys`. */
  #proposals(keys: string[]): TurnProposal[] {
    return this.turns.flatMap((t) => t.proposals).filter((p) => keys.includes(p.key));
  }

  #saveThread() {
    if (this.#thread) saveThread(this.job, this.#thread, $state.snapshot(this.turns));
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
    // Another review (master, mode, a re-run) has a thread of its own.
    if (review.id !== this.#thread) {
      this.#thread = review.id;
      this.turns = loadThread(this.job, review.id, (key) => find(review, key) !== undefined);
      // A thread carried over from the review this one replaced is this one's from now on.
      this.#saveThread();
    }
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

/** Where a proposal stands in the review as it is now: undo can take it away, and the plan can turn it on or off. */
export function proposalState(review: ReviewView, p: TurnProposal): ProposalState {
  const change = find(review, p.key);
  if (!change) return p.rejected ? "rejected" : "gone";
  return change.state === "off" ? "pending" : "accepted";
}
