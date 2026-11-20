<!--
  The live interview for one round (DESIGN.md, Job → Interview: live interview), a sibling of `Practice`: the
  interviewer asks the prep's questions in order, as written, and after each answer one model call (the Coaching role)
  decides a follow-up on what you said, or none (two at most per question); then the next question. No feedback, no
  grade, no Nudge or Again until it ends. You answer in the box at the foot, typed or out loud (**Speak**, `⌘D`, as
  Practice's). While she reads your answer the box is shut and the clock waits: `Thinking` in her place with **Take
  back** (`Esc`: the call stops and your answer comes back to the box). A call that fails sits in the thread where her
  turn would be, with **Try again** (`⌘R`) and **End here**; the strip says Paused. **Next question** (`N`) skips the
  rest of this question. **End interview** (`⌘.`, the toolbar's) ends it at once, unless the box holds a draft: then
  the foot asks inline (Keep going, End without it, Send and end). Past the last question it ends by itself. Ended,
  the column is the run's readback (`LiveReadback`). The strip holds the session: the clock (waits excluded, kept with
  the run when it ends), who asks, a cell per prep question, and which is being asked.
-->
<script lang="ts" module>
  /** The clock of each run opened this launch, so leaving for the prep and coming back keeps it. */
  const clocks = new Map<string, number>();
</script>

<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import SkipForward from "@lucide/svelte/icons/skip-forward";
  import UserRound from "@lucide/svelte/icons/user-round";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import {
    appError,
    cancel,
    liveAnswer,
    liveEnd,
    liveRetry,
    liveSession,
    liveSkip,
    liveTakeBack,
    practiceHints,
    type LiveSession,
    type PrepQuestion,
    type PrepView,
    type Round,
    type SentLine,
    type Spoken,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { interviewer, kindLabel } from "$lib/rounds";
  import { Speech } from "$lib/speech.svelte";
  import {
    Button,
    Icon,
    KeyFooter,
    type KeyCommand,
    SpeakButton,
    Textarea,
    Thinking,
    tip,
    toast,
  } from "$lib/ui";
  import LiveReadback from "./LiveReadback.svelte";
  import LiveThread from "./LiveThread.svelte";
  import { clockText, currentTurn } from "./live";

  interface Props {
    job: string;
    company: string;
    round: Round;
    prep: PrepView;
    lines: SentLine[];
    /** The run (bindable: every step brings it back). */
    session: LiveSession;
    /** The question the desk marks (bindable): the one being asked, or the readback's card last opened. */
    question: string | null;
    /** The screen's keys, after the session's own in Commands. */
    keys: KeyCommand[];
    /** Back to the prep (the readback's Esc). */
    onback: () => void;
    /** Practice a question the run didn't reach (the readback's Practice it). */
    onpractice: (question: PrepQuestion) => void;
  }

  let {
    job,
    company,
    round,
    prep,
    lines,
    session = $bindable(),
    question = $bindable(),
    keys,
    onback,
    onpractice,
  }: Props = $props();

  const who = $derived(interviewer(round, company));
  const ended = $derived(session.endedAt !== null);
  const turns = $derived(session.turns);
  const current = $derived(currentTurn(turns));
  /** The question being asked: the current turn's, else the last turn's. */
  const asking = $derived(current?.question ?? turns.at(-1)?.question ?? null);
  const order = $derived(prep.questions.findIndex((q) => q.id === asking));
  const asked = $derived(new Set(turns.filter((t) => t.kind === "ask").map((t) => t.question)));
  const answers = $derived(turns.filter((t) => t.answer !== null).length);

  let answer = $state("");
  /** Her turn is being read (`performance.now()` when the call started): the box is shut, the clock waits. */
  let stepping = $state<number | null>(null);
  /** The call in flight, so Take back and End wait for it to stop. */
  let pending: Promise<unknown> | null = null;
  /** Why her last turn didn't come; with `session.waiting`, the run is paused on it. */
  let failed = $state<string | null>(null);
  /** End was pressed with a draft in the box: the foot asks. */
  let guard = $state(false);
  let ending = $state(false);
  let skipping = $state(false);
  const paused = $derived(!ended && stepping === null && (session.waiting || failed !== null));
  /** The box takes an answer: a question is asked and nothing is in the way. */
  const open = $derived(!ended && current !== undefined && stepping === null && !paused && !ending);

  let field = $state<Textarea>();
  let thread = $state<HTMLElement>();

  // ── The clock: it runs while a question waits on you, never while she reads or a call failed ──
  const running = $derived(!ended && stepping === null && !paused && !ending);
  /** Time counted before the current stretch: kept this launch, else the run's own, else its turns' span. */
  let base = $state(
    untrack(() => {
      const kept = clocks.get(session.id) ?? session.activeMs;
      if (kept !== null && kept !== undefined) return kept;
      const last = session.turns.at(-1)?.at;
      return last ? Math.max(0, Date.parse(last) - Date.parse(session.startedAt)) : 0;
    }),
  );
  let since = $state<number | null>(null);
  let at = $state(0);
  $effect(() => {
    if (!running) return;
    const id = untrack(() => session.id);
    const start = performance.now();
    since = start;
    at = start;
    const timer = setInterval(() => (at = performance.now()), 250);
    return () => {
      clearInterval(timer);
      base += performance.now() - start;
      since = null;
      clocks.set(id, base);
    };
  });
  const elapsed = $derived(base + (since !== null ? Math.max(0, at - since) : 0));

  /** Speak: the answer said out loud into the box (Practice's). */
  const speech = new Speech({ get: () => answer, set: (text) => (answer = text) });
  let terms = $state<string[] | null>(null);
  const hints = $derived(terms ?? [company, ...prep.gaps.map((g) => g.term)]);
  const speak = () => void speech.toggle(hints);
  $effect(() => {
    const id = round.id;
    terms = null;
    void practiceHints(id)
      .then((h) => {
        if (round.id === id) terms = h;
      })
      .catch(() => undefined);
  });
  onMount(() => {
    void speech.refresh();
    void scrollEnd();
    return () => speech.cancel();
  });
  $effect(() => {
    if (!answer.trim() && !speech.listening) speech.reset();
  });
  // The desk marks the question being asked.
  $effect(() => {
    if (!ended && asking) question = asking;
  });
  // A question asked opens the box.
  $effect(() => {
    if (open) untrack(() => field?.focus());
  });

  async function scrollEnd() {
    await tick();
    thread?.scrollTo({ top: thread.scrollHeight });
  }

  /** Runs the step owed (an answer's, or a retry): her next turn, a follow-up or the next question, or the end. */
  async function step(run: () => Promise<LiveSession>) {
    stepping = performance.now();
    failed = null;
    const call = run();
    pending = call;
    void scrollEnd();
    try {
      session = await call;
      await record();
    } catch (e) {
      const error = appError(e);
      if (error.kind !== "cancelled") {
        failed = error.message;
        try {
          session = await liveSession(session.id);
        } catch {
          session = { ...session, waiting: true };
        }
      }
    } finally {
      if (pending === call) pending = null;
      stepping = null;
      void scrollEnd();
    }
  }

  /** Ended by itself (past the last question): the clock is kept with the run. */
  async function record() {
    if (session.endedAt === null || session.activeMs !== null) return;
    await tick();
    try {
      session = await liveEnd(session.id, Math.round(base));
    } catch {
      // The run is ended either way; only its length goes unsaved.
    }
  }

  /** Puts the answer on the current turn at once (the backend keeps it before it calls), then asks for her turn. */
  async function sendText(text: string, spoken: Spoken | null) {
    const turn = current;
    if (!turn) return;
    session = {
      ...session,
      turns: session.turns.map((t) => (t.seq === turn.seq ? { ...t, answer: text, spoken } : t)),
    };
    await step(() => liveAnswer(session.id, text, spoken));
  }

  async function send() {
    // Opening: nothing heard yet to send. Listening or closing: send once the last words land.
    if (speech.opening) return;
    await speech.stop();
    const text = answer.trim();
    if (!text || !open) return;
    const spoken = speech.spoken;
    answer = "";
    speech.reset();
    (document.activeElement as HTMLElement | null)?.blur();
    await sendText(text, spoken);
  }

  /** Take back: the call stops and your answer comes back to the box, its turn unanswered again. */
  async function takeBack() {
    if (stepping === null) return;
    const call = pending;
    await cancel(job).catch(() => undefined);
    await call?.catch(() => undefined);
    try {
      const back = await liveTakeBack(session.id);
      session = back.session;
      answer = back.answer;
      failed = null;
      await tick();
      field?.focus();
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  async function retry() {
    if (!paused) return;
    await step(() => liveRetry(session.id));
  }

  /** Next question: the rest of this one (its follow-ups, or the question itself) is skipped. */
  async function skip() {
    if (!open || skipping) return;
    speech.cancel();
    skipping = true;
    try {
      session = await liveSkip(session.id);
      await record();
      void scrollEnd();
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      skipping = false;
    }
  }

  /** The toolbar's End interview (`⌘.`): ends at once, unless the box holds a draft. */
  export function end() {
    if (ended || ending) return;
    if (guard) return;
    if (open && answer.trim()) {
      guard = true;
      return;
    }
    void finish();
  }

  /** Ends the run: a call in flight stops (the answer it read is kept, without a follow-up), the clock is kept.
   *  `last` (Send and end): the draft, kept on the current turn with no call. */
  async function finish(last: { text: string; spoken: Spoken | null } | null = null) {
    guard = false;
    ending = true;
    speech.cancel();
    try {
      if (stepping !== null) {
        const call = pending;
        await cancel(job).catch(() => undefined);
        await call?.catch(() => undefined);
      }
      await tick();
      session = await liveEnd(session.id, Math.round(base), last?.text ?? null, last?.spoken ?? null);
      clocks.delete(session.id);
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      ending = false;
    }
  }

  function keepGoing() {
    guard = false;
    field?.focus();
  }

  async function sendAndEnd() {
    if (speech.opening) return;
    await speech.stop();
    const text = answer.trim();
    guard = false;
    const last = text && open ? { text, spoken: speech.spoken } : null;
    answer = "";
    speech.reset();
    if (session.endedAt === null) await finish(last);
  }

  function dropAndEnd() {
    answer = "";
    speech.reset();
    void finish();
  }

  function onFieldKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void (guard ? sendAndEnd() : send());
    } else if (e.key === "Escape") {
      e.stopPropagation();
      if (guard) return keepGoing();
      if (speech.listening) return void speech.stop();
      (e.currentTarget as HTMLElement).blur();
    }
  }

  const interactive = (target: EventTarget | null) =>
    target instanceof HTMLElement && target.closest("button, a, input, textarea, select") !== null;

  const KEYS: KeyCommand[] = $derived([
    guard
      ? { keys: ["⏎"], label: "Send and end", run: () => void sendAndEnd() }
      : {
          keys: ["⏎"],
          label: "Send your answer",
          run: () => void send(),
          disabled: !answer.trim() || !open || speech.opening,
        },
    {
      keys: ["⌘D"],
      label: speech.listening ? "Stop speaking" : "Speak your answer",
      run: speak,
      disabled: !open || speech.busy || !!speech.off,
    },
    { keys: ["N"], label: "Next question", run: () => void skip(), disabled: !open || skipping },
    ...(stepping !== null ? [{ keys: ["Esc"], label: "Take back your answer", run: () => void takeBack() }] : []),
    ...(guard ? [{ keys: ["Esc"], label: "Keep going", run: keepGoing }] : []),
    ...(paused ? [{ keys: ["⌘R"], label: "Try again", run: () => void retry() }] : []),
    { keys: ["⌘."], label: "End interview", run: end, disabled: ending },
    ...keys,
  ]);

  $effect(() => {
    if (ended) return;
    return bindKeys({
      n: (e) => (interactive(e.target) || !open ? false : void skip()),
      "mod+d": () => (!open || speech.off ? undefined : speak()),
      "mod+r": () => (paused ? void retry() : false),
      "mod+.": () => end(),
      Enter: (e) => (guard && !interactive(e.target) ? void sendAndEnd() : false),
      // Esc never ends a session: it takes an answer back while she reads, answers the guard, or closes the mic.
      Escape: () => {
        if (stepping !== null && !ending) return void takeBack();
        if (guard) return keepGoing();
        if (speech.listening) return void speech.stop();
        return false;
      },
    });
  });

  const hint = $derived.by(() => {
    if (stepping !== null) return "The clock waits. Take back puts your answer back in the box.";
    if (paused) return `Paused at ${clockText(elapsed)} until ${who.name}’s turn comes. Nothing you said is lost.`;
    if (speech.listening) return "Listening on this Mac. ⌘D or Send closes the mic; then the words are yours to fix.";
    return speech.off
      ? "No feedback until you end. Answer as you would out loud."
      : "No feedback until you end. Say it with ⌘D, or type it.";
  });
</script>

{#if ended}
  <LiveReadback {job} {company} {round} {prep} {lines} bind:session bind:question {keys} {onback} {onpractice} />
{:else}
  <div class="pin">
    <div class="strip">
      <Icon icon={UserRound} size={16} />
      <span class="strip__n">Live</span>
      <span class={["strip__clock num", !running && "strip__clock--held"]}>{clockText(elapsed)}</span>
      <span class="strip__who">{who.asker} · {kindLabel(round.kind)}</span>
      {#if paused}<span class="meta">Paused</span>{/if}
      <span class="meter" aria-hidden="true">
        {#each prep.questions as q (q.id)}
          <span class={["cell", asked.has(q.id) && q.id !== asking && "cell--done", q.id === asking && "cell--now"]}
          ></span>
        {/each}
      </span>
      <span class="meta num">Question {order + 1} of {session.questions}</span>
    </div>
  </div>

  <div class="thread" bind:this={thread}>
    <LiveThread {turns} {prep} total={session.questions} asker={who.asker}>
      {#snippet tail()}
        {#if stepping !== null}
          <article class="turn">
            <p class="who">
              <Icon icon={UserRound} size={16} /><span class="who__name">{who.asker}</span><span class="meta"
                >Usually 4–12 s</span
              >
            </p>
            <Thinking
              label="{who.name} is reading your answer…"
              startedAt={stepping}
              cancelLabel="Take back"
              cancelKbd="Esc"
              oncancel={ending ? undefined : () => void takeBack()}
            />
          </article>
        {:else if paused}
          <article class="turn">
            <p class="who">
              <Icon icon={UserRound} size={16} /><span class="who__name">{who.asker}</span><span class="meta"
                >Next turn</span
              >
            </p>
            <div class="failed" role="alert">
              <p class="meta problem">
                <Icon icon={CircleAlert} size={14} weight="medium" /><span
                  >{failed ?? "The call for her next turn didn’t finish."} Your answer is kept · the clock is paused.</span
                >
              </p>
              <p class="acts">
                <Button size="sm" icon={RotateCcw} kbd="⌘R" onclick={() => void retry()}>Try again</Button>
                <Button variant="ghost" size="sm" disabled={ending} onclick={() => void finish()}>End here</Button>
                <span class="meta"
                  >End here reads back the {answers === 1 ? "answer" : `${answers} answers`} you gave.</span
                >
              </p>
            </div>
          </article>
        {/if}
      {/snippet}
    </LiveThread>
  </div>

  <div class="foot">
    {#if guard}
      <div class="foot__row" role="group" aria-label="End the interview">
        <span class="ask">Your draft isn’t sent.</span>
        <Button variant="ghost" size="sm" kbd="Esc" onclick={keepGoing}>Keep going</Button>
        <Button size="sm" onclick={dropAndEnd}>End without it</Button>
        <Button variant="primary" size="sm" kbd="⏎" onclick={() => void sendAndEnd()}>Send and end</Button>
      </div>
    {:else}
      <div class="foot__row">
        <span class="meta next">{hint}</span>
        <Button
          variant="ghost"
          size="sm"
          icon={SkipForward}
          kbd="N"
          disabled={!open || skipping}
          onclick={() => void skip()}>Next question</Button
        >
      </div>
    {/if}
    <div class="composer">
      <Textarea
        bind:this={field}
        bind:value={answer}
        placeholder={!open && !guard
          ? "Your next answer goes here once she asks"
          : speech.listening
            ? "Listening…"
            : "Your answer…"}
        rows={3}
        aria-label="Your answer"
        disabled={!open}
        readonly={speech.listening}
        onkeydown={onFieldKey}
      />
      <div class="composer__bar">
        <KeyFooter keys={KEYS} bare />
        <div class="composer__send">
          <SpeakButton
            openedAt={speech.openedAt}
            busy={!open || speech.busy}
            off={speech.off}
            kbd="⌘D"
            onclick={speak}
          />
          <Button
            variant="primary"
            size="sm"
            square
            icon={ArrowUp}
            aria-label="Send your answer"
            {@attach tip("Send your answer (⏎)")}
            disabled={!answer.trim() || !open || guard || speech.opening}
            onclick={() => void send()}
          />
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .pin {
    flex: none;
  }
  .strip {
    display: flex;
    align-items: center;
    gap: var(--space-2-5);
    height: 48px;
    padding-inline: var(--space-6) var(--space-5);
    background: var(--paper);
    box-shadow: inset 0 -1px 0 var(--line);
    white-space: nowrap;
  }
  .strip :global(.icon) {
    flex: none;
    color: var(--ink-3);
  }
  .strip__n {
    flex: none;
    font-weight: var(--weight-medium);
  }
  .strip__clock {
    flex: none;
    color: var(--ink-2);
  }
  /* The clock waits while she reads or a call failed: ink 3, not counting. */
  .strip__clock--held {
    color: var(--ink-3);
  }
  .strip__who {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--ink-2);
  }
  /* Asking isn't points: the meter is ink, the question being asked outlined in the accent. */
  .meter {
    display: inline-flex;
    flex: none;
    gap: 2px;
  }
  .cell {
    width: 6px;
    height: 12px;
    border-radius: 1px;
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .cell--done {
    background: var(--ink-3);
    box-shadow: none;
  }
  .cell--now {
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .thread {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    padding: var(--space-5) var(--space-6);
  }
  .turn p {
    margin: 0;
  }
  .turn .who {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-1-5);
  }
  .who :global(.icon) {
    color: var(--ink-3);
  }
  .who__name {
    font-weight: var(--weight-medium);
  }
  /* Her turn failed: where it would be, as the prep's failure (danger meta, Try again); never a toast. */
  .failed {
    display: grid;
    justify-items: start;
    gap: var(--space-2);
  }
  .problem {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    color: var(--danger);
  }
  .problem :global(.icon) {
    flex: none;
    margin-top: 1px;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
  }
  .foot {
    flex: none;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-2);
    padding: var(--space-2) var(--space-5) var(--space-4);
    box-shadow: inset 0 1px 0 var(--line);
    background: var(--paper);
  }
  .foot__row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-sm);
  }
  .next,
  .ask {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ask {
    font-weight: var(--weight-medium);
  }
  .composer {
    display: grid;
    gap: var(--space-1);
    padding: var(--space-1);
    border-radius: var(--radius-lg);
    border: 1px solid var(--input-border);
    background: var(--surface);
  }
  .composer :global(textarea) {
    border: 0;
    box-shadow: none;
    background: none;
  }
  .composer:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px oklch(from var(--accent) l c h / 0.18);
  }
  .composer__bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }
  .composer__send {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
</style>
