<!--
  A live run's readback (DESIGN.md, Job → Interview: live interview), once the run has ended. Each answered question
  is read in turn, one model call each (Practice's reading, its follow-ups and your replies with it), kept as a
  practice answer of the round, so it counts as practiced: `Thinking` says which ("Reading answer 2 of 6…") with
  Cancel, which keeps the transcript and what's read; the rest is read the next time the run is opened. The strip
  switches **Readback · Transcript** (the thread, read-only). The readback: a sentence summary (what to settle before
  the round, the one amber row; what's new to gliide; which answers tell a story; the transcript kept), each row a way
  to its card, then a card per question: `FeedbackCard` with a head that folds it (the question, your first words, its
  follow-ups, how long you spoke) and **She asked** for the follow-ups as asked; cards with something to act on (a
  mismatch, a claim not kept, a story not saved) start open, the rest folded. Then the questions not answered, each
  with **Practice it**. Keep as fact and Save as story (`S`, the newest) as in Practice. Never a grade.
-->
<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import BookOpen from "@lucide/svelte/icons/book-open";
  import Check from "@lucide/svelte/icons/check";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Plus from "@lucide/svelte/icons/plus";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import ScrollText from "@lucide/svelte/icons/scroll-text";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import {
    appError,
    cancel,
    keepClaim,
    liveRead,
    saveStory,
    type Feedback,
    type LiveSession,
    type NewClaim,
    type PracticeTurn,
    type PrepQuestion,
    type PrepView,
    type Round,
    type SentLine,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { interviewer } from "$lib/rounds";
  import { spokenTime } from "$lib/speech.svelte";
  import {
    Button,
    Icon,
    KeyFooter,
    type KeyCommand,
    Segmented,
    Thinking,
    toast,
    WorkingRing,
  } from "$lib/ui";
  import FeedbackCard from "./FeedbackCard.svelte";
  import LiveThread from "./LiveThread.svelte";
  import { answered, runDay, runLength } from "./live";

  interface Props {
    job: string;
    company: string;
    round: Round;
    prep: PrepView;
    lines: SentLine[];
    /** The ended run (bindable: each answer read joins its `read`). */
    session: LiveSession;
    /** The question the desk marks (bindable): the card last opened. */
    question: string | null;
    keys: KeyCommand[];
    onback: () => void;
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

  const byId = $derived(new Map(lines.map((l) => [l.id, l])));
  const who = $derived(interviewer(round, company));
  const turns = $derived(session.turns);
  /** The questions you answered, in the order she asked them. */
  const answeredIds = $derived([...new Set(turns.filter((t) => t.answer !== null).map((t) => t.question))]);
  const readBy = $derived(new Map(session.read.map((t) => [t.question, t])));
  const unread = $derived(answeredIds.filter((q) => !readBy.has(q)));
  /** The readback's cards, in the order she asked. */
  const cards = $derived(answeredIds.map((q) => readBy.get(q)).filter((t) => t !== undefined));
  const notAnswered = $derived(prep.questions.filter((q) => !answered(turns, q.id)));
  const follows = $derived(turns.filter((t) => t.kind === "follow").length);
  const number = (q: string) => prep.questions.findIndex((x) => x.id === q) + 1 || answeredIds.indexOf(q) + 1;
  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

  let view = $state<"readback" | "transcript">("readback");
  let scroller = $state<HTMLElement>();

  // ── Reading, one answered question at a time ──
  /** The answer being read: its stage line and when it started. */
  let reading = $state<{ label: string; startedAt: number } | null>(null);
  /** Why reading stopped short: cancelled, or what failed. */
  let stopped = $state<{ cancelled: boolean; message: string } | null>(null);
  let alive = true;

  async function readAll() {
    if (reading) return;
    stopped = null;
    for (const q of untrack(() => unread)) {
      if (!alive) return;
      const n = answeredIds.indexOf(q) + 1;
      reading = { label: `Reading answer ${n} of ${answeredIds.length}…`, startedAt: performance.now() };
      try {
        const turn = await liveRead(session.id, q);
        if (!alive) return;
        session = { ...session, read: [...session.read.filter((t) => t.question !== q), turn] };
      } catch (e) {
        const error = appError(e);
        stopped = { cancelled: error.kind === "cancelled", message: error.message };
        break;
      }
    }
    reading = null;
  }

  onMount(() => {
    if (unread.length) void readAll();
  });
  // Leaving stops the read in flight; what's left is read the next time the run is opened.
  onDestroy(() => {
    alive = false;
    if (reading) void cancel(job).catch(() => undefined);
  });

  // ── Cards: open while there's something to act on, else folded to their head ──
  let kept = $state(new Set<string>());
  /** Turns saved as a story this visit: the turn's id → the story's. */
  let saved = $state(new Map<string, string>());
  let saving = $state(false);
  /** Each card's fold, set when it first shows (so keeping its last claim doesn't fold it under you). */
  let folded = $state(new Map<string, boolean>());

  const actionable = (t: PracticeTurn) =>
    t.feedback.mismatches.length > 0 ||
    t.feedback.newClaims.some((c) => !kept.has(c.text)) ||
    (t.feedback.story !== null && !saved.has(t.id));
  $effect.pre(() => {
    const fresh = cards.filter((t) => !untrack(() => folded.has(t.id)));
    if (fresh.length) untrack(() => (folded = new Map([...folded, ...fresh.map((t) => [t.id, !actionable(t)] as const)])));
  });
  const isFolded = (t: PracticeTurn) => folded.get(t.id) ?? !actionable(t);
  function fold(t: PracticeTurn, value: boolean) {
    folded = new Map(folded).set(t.id, value);
    if (!value) question = t.question;
  }

  /** Opens a card and brings it into view (a summary row's way to it). */
  async function goTo(t: PracticeTurn | undefined) {
    if (!t) return;
    view = "readback";
    fold(t, false);
    await tick();
    scroller?.querySelector(`[data-question="${CSS.escape(t.question)}"]`)?.scrollIntoView({ block: "start" });
  }

  /** "Thin task, missing result": the STAR parts an answer is short on, for a folded card's meta. */
  function short(f: Feedback): string | null {
    const parts = (["situation", "task", "action", "result"] as const)
      .filter((p) => f[p] !== "yes")
      .map((p) => `${p} ${f[p] === "thin" ? "thin" : "missing"}`);
    return parts.length ? parts.join(", ") : null;
  }

  /** A card's head meta: "“At Acme I wrote…” · 1 follow-up · said in 24 s · task thin". */
  function headMeta(t: PracticeTurn): string {
    const mine = turns.filter((x) => x.question === t.question);
    const first = mine.find((x) => x.answer !== null)?.answer ?? t.answer;
    const words = first.split(/\s+/).filter(Boolean);
    const quote = words.length > 8 ? `${words.slice(0, 8).join(" ")}…` : words.join(" ");
    const asked = mine.filter((x) => x.kind === "follow").length;
    const ms = mine.reduce((sum, x) => sum + (x.spoken?.ms ?? 0), 0);
    return [
      `“${quote}”`,
      asked ? plural(asked, "follow-up", "follow-ups") : "no follow-up",
      ms ? `said in ${spokenTime(ms)}` : null,
      short(t.feedback),
    ]
      .filter(Boolean)
      .join(" · ");
  }

  const askedOn = (q: string) => turns.filter((t) => t.question === q && t.kind === "follow").map((t) => t.asked);

  async function keep(claim: NewClaim) {
    try {
      await keepClaim(round.id, claim.entry, claim.text);
      kept = new Set([...kept, claim.text]);
      toast(`Kept as a fact of ${claim.entryName}`);
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  /** The newest card with a story to save that isn't saved yet: what `S` saves. */
  const saveable = $derived(cards.findLast((t) => t.feedback.story && !saved.has(t.id)));
  const storyHref = (id: string) => `/bank/stories/${encodeURIComponent(id)}`;

  async function saveAsStory(turn: PracticeTurn | undefined) {
    const draft = turn?.feedback.story;
    if (!turn || !draft || saved.has(turn.id) || saving) return;
    saving = true;
    try {
      const story = await saveStory(draft);
      saved = new Map([...saved, [turn.id, story.id]]);
      toast("Saved as a story", { action: { label: "Open", run: () => goto(storyHref(story.id)) } });
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    } finally {
      saving = false;
    }
  }

  // ── The summary: counts in sentences, never a grade ──
  const mismatches = $derived(cards.flatMap((t) => t.feedback.mismatches.map((m) => ({ t, m }))));
  const claims = $derived(cards.flatMap((t) => t.feedback.newClaims.map((c) => ({ t, c }))));
  const keptCount = $derived(claims.filter(({ c }) => kept.has(c.text)).length);
  const stories = $derived(cards.filter((t) => t.feedback.story !== null));
  const savedCount = $derived(stories.filter((t) => saved.has(t.id)).length);

  /** "before Tuesday" while the round is to come (within the week, else its day), else nothing. */
  const before = $derived.by(() => {
    if (!round.startsAt) return "";
    const d = new Date(round.startsAt);
    const ahead = d.getTime() - Date.now();
    if (ahead <= 0) return "";
    if (ahead < 6 * 86_400_000) return ` before ${d.toLocaleDateString(undefined, { weekday: "long" })}`;
    return ` before ${d.toLocaleDateString(undefined, { month: "short", day: "numeric" })}`;
  });
  const questionList = (ns: number[]) => {
    const unique = [...new Set(ns)].sort((a, b) => a - b);
    if (unique.length === 1) return `Question ${unique[0]}`;
    return `Questions ${unique.slice(0, -1).join(", ")} and ${unique.at(-1)}`;
  };

  const endedAt = $derived(
    session.endedAt
      ? new Date(session.endedAt).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })
      : "",
  );
  const length = $derived(session.activeMs !== null ? runLength(session.activeMs) : null);

  const KEYS: KeyCommand[] = $derived([
    ...(saveable ? [{ keys: ["S"], label: "Save as story", run: () => void saveAsStory(saveable), disabled: saving }] : []),
    ...(reading ? [{ keys: ["Esc"], label: "Cancel reading", run: () => void cancel(job) }] : []),
    { keys: ["Esc"], label: "Back to prep", run: onback },
    ...keys,
  ]);

  const interactive = (target: EventTarget | null) =>
    target instanceof HTMLElement && target.closest("button, a, input, textarea, select") !== null;

  $effect(() =>
    bindKeys({
      s: (e) => (interactive(e.target) || !saveable ? false : void saveAsStory(saveable)),
      Escape: (e) => {
        if (reading) return void cancel(job);
        if (interactive(e.target)) return false;
        onback();
      },
    }),
  );
</script>

<div class="pin">
  <div class="strip">
    <Icon icon={ScrollText} size={16} />
    <span class="strip__n">Readback</span>
    <span class="strip__who num">Live run, {runDay(session.startedAt)}{length ? ` · ${length}` : ""}</span>
    <Segmented
      options={[
        { value: "readback" as const, label: "Readback" },
        { value: "transcript" as const, label: "Transcript" },
      ]}
      bind:value={view}
      label="Show"
    />
  </div>
</div>

{#if view === "transcript"}
  <div class="scroll" bind:this={scroller}>
    <div class="transcript">
      <LiveThread {turns} {prep} total={session.questions} asker={who.asker} />
    </div>
    <KeyFooter keys={KEYS} />
  </div>
{:else if reading && cards.length === 0}
  <div class="pane">
    <Thinking center label={reading.label} startedAt={reading.startedAt} oncancel={() => void cancel(job)} />
    <WorkingRing on />
  </div>
  <div class="foot">
    <span class="meta">The transcript is kept. Cancel keeps it too; the rest is read the next time you open this run.</span>
  </div>
{:else}
  <div class="scroll" bind:this={scroller}>
    <div class="readback">
      <div class="intro">
        <h2 class="heading">
          {plural(answeredIds.length, "question", "questions")}{follows ? `, ${plural(follows, "follow-up", "follow-ups")}` : ""}
        </h2>
        <p class="meta num">
          Ended {endedAt}{length ? ` · ${length}` : ""} · each answer read against the resume you sent · no grade
        </p>
      </div>

      {#if cards.length}
        <ul class="summary">
          {#if mismatches.length}
            {@const first = mismatches[0]}
            <li>
              <button type="button" class="sum sum--warn" onclick={() => void goTo(first.t)}>
                <Icon icon={TriangleAlert} size={16} weight="medium" />
                <span class="sum__text">
                  <span class="sum__title"
                    >{plural(mismatches.length, "thing", "things")} to settle{before}</span
                  >
                  <span class="meta"
                    >{#if mismatches.length === 1}You said {first.m.said}; the resume you sent says {first.m.resume}.
                      {questionList([number(first.t.question)])}.{:else}{questionList(
                        mismatches.map(({ t }) => number(t.question)),
                      )}.{/if}</span
                  >
                </span>
                <Icon icon={ChevronRight} size={14} />
              </button>
            </li>
          {/if}
          {#if claims.length}
            <li>
              <button
                type="button"
                class="sum"
                onclick={() => void goTo((claims.find(({ c }) => !kept.has(c.text)) ?? claims[0]).t)}
              >
                <Icon icon={Plus} size={16} />
                <span class="sum__text">
                  <span class="sum__title">{plural(claims.length, "thing", "things")} new to gliide</span>
                  <span class="meta"
                    >{keptCount === claims.length
                      ? "All kept as facts, your answers as their source."
                      : keptCount
                        ? `${keptCount} kept · ${claims.length - keptCount} to keep, below.`
                        : "Keep them as facts below, your answers as their source."}</span
                  >
                </span>
                <Icon icon={ChevronRight} size={14} />
              </button>
            </li>
          {/if}
          {#if stories.length}
            <li>
              <button
                type="button"
                class="sum"
                onclick={() => void goTo(stories.find((t) => !saved.has(t.id)) ?? stories[0])}
              >
                <Icon icon={BookOpen} size={16} />
                <span class="sum__text">
                  <span class="sum__title"
                    >{stories.length === 1 ? "1 answer tells a story" : `${stories.length} answers tell a story`}</span
                  >
                  <span class="meta"
                    >{savedCount === stories.length
                      ? stories.length === 1
                        ? "Saved as a story."
                        : "Saved as stories."
                      : stories.length === 1
                        ? "Save it below, or press S."
                        : "Save them below; the newest with S."}</span
                  >
                </span>
                <Icon icon={ChevronRight} size={14} />
              </button>
            </li>
          {/if}
          <li>
            <button type="button" class="sum" onclick={() => (view = "transcript")}>
              <Icon icon={ScrollText} size={16} />
              <span class="sum__text">
                <span class="sum__title">Transcript kept with the round</span>
                <span class="meta">Every question, follow-up and answer, as said.</span>
              </span>
              <Icon icon={ChevronRight} size={14} />
            </button>
          </li>
        </ul>
      {/if}

      {#each cards as t (t.id)}
        {#snippet story()}
          {@const id = saved.get(t.id)}
          {#if id}
            <a class="meta saved" href={storyHref(id)}><Icon icon={Check} size={14} />Saved as a story</a>
          {:else}
            <Button
              variant="ghost"
              size="sm"
              icon={BookOpen}
              kbd={t === saveable ? "S" : undefined}
              disabled={saving}
              onclick={() => saveAsStory(t)}>Save as story</Button
            >
          {/if}
        {/snippet}
        <div class="card" data-question={t.question}>
          <FeedbackCard
            feedback={t.feedback}
            line={(id) => byId.get(id)}
            {kept}
            onkeep={keep}
            grounded="Only your answers and what you sent"
            actions={t.feedback.story ? story : undefined}
            head={{ n: number(t.question), question: prep.questions.find((q) => q.id === t.question)?.text ?? t.asked, meta: headMeta(t) }}
            bind:collapsed={() => isFolded(t), (v) => fold(t, v)}
            asked={askedOn(t.question)}
          />
        </div>
      {/each}

      {#if reading}
        <div class="reading">
          <Thinking label={reading.label} startedAt={reading.startedAt} oncancel={() => void cancel(job)} />
        </div>
      {:else if stopped && unread.length}
        <div class="stopped">
          {#if stopped.cancelled}
            <p class="meta">
              Read {cards.length} of {answeredIds.length}. The rest is read the next time you open this run.
            </p>
          {:else}
            <p class="meta problem" role="alert">{stopped.message}</p>
          {/if}
          <Button size="sm" icon={RotateCcw} onclick={() => void readAll()}
            >{stopped.cancelled ? "Read the rest" : "Try again"}</Button
          >
        </div>
      {/if}

      {#if notAnswered.length}
        <p class="label group">Not answered <span class="num">{notAnswered.length}</span></p>
        <ul class="rest">
          {#each notAnswered as q (q.id)}
            <li class="rest__row">
              <span class="rest__n num">{prep.questions.indexOf(q) + 1}</span>
              <span class="rest__q">{q.text}</span>
              <Button variant="ghost" size="sm" onclick={() => onpractice(q)}>Practice it</Button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
    <KeyFooter keys={KEYS} />
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
  .strip > :global(.icon) {
    flex: none;
    color: var(--ink-3);
  }
  .strip__n {
    flex: none;
    font-weight: var(--weight-medium);
  }
  .strip__who {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--ink-2);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    display: flex;
    flex-direction: column;
  }
  .transcript,
  .readback {
    flex: 1 0 auto;
    padding: var(--space-5) var(--space-6) var(--space-8);
  }
  .readback {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .readback > :global(*) {
    flex: none;
  }
  /* The readback being written before any answer is read: the pane wears the ring, its stage centered. */
  .pane {
    position: relative;
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
  }
  .foot {
    flex: none;
    padding: var(--space-2) var(--space-5) var(--space-4);
    box-shadow: inset 0 1px 0 var(--line);
    background: var(--paper);
  }
  .intro {
    display: grid;
    gap: var(--space-0-5);
    padding-inline: var(--space-1);
  }
  .intro p {
    margin: 0;
  }
  .heading {
    margin: 0;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    font-weight: var(--weight-medium);
    text-wrap: balance;
  }
  /* The summary: a row a sentence (icon, 500 words over meta), each a way to its card; the warn row the one amber. */
  .summary {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-border);
    overflow: hidden;
  }
  .summary li + li {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .sum {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) 14px;
    column-gap: var(--space-2-5);
    align-items: start;
    width: 100%;
    padding: var(--space-2-5) var(--space-3);
    border: 0;
    background: none;
    font: inherit;
    color: inherit;
    text-align: start;
    cursor: default;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .sum:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  @media (hover: hover) {
    .sum:hover {
      background: var(--hover);
    }
  }
  .sum > :global(.icon) {
    margin-top: 2px;
    color: var(--ink-3);
  }
  .sum > :global(.icon:last-child) {
    margin-top: 3px;
    color: var(--ink-4);
  }
  .sum--warn > :global(.icon:first-child) {
    color: var(--warn);
  }
  .sum__text {
    display: grid;
    gap: var(--space-0-5);
    min-width: 0;
  }
  .sum__title {
    font-weight: var(--weight-medium);
  }
  .card {
    scroll-margin-top: var(--space-3);
  }
  .reading,
  .stopped {
    display: grid;
    justify-items: start;
    gap: var(--space-2);
    padding-inline: var(--space-1);
  }
  .stopped p {
    margin: 0;
  }
  .problem {
    color: var(--danger);
  }
  /* Saved as a story: the row says so, a gain check, and opens it. */
  .saved {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--ink-2);
    text-decoration: none;
  }
  .saved :global(.icon) {
    color: var(--gain-text);
  }
  @media (hover: hover) {
    .saved:hover {
      color: var(--ink);
    }
  }
  .group {
    margin: var(--space-3) var(--space-1) 0;
    color: var(--ink-3);
  }
  .rest {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .rest__row {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    column-gap: var(--space-2);
    align-items: center;
    padding: var(--space-1-5) var(--space-1-5) var(--space-1-5) var(--space-3);
    border-radius: var(--radius-md);
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  @media (hover: hover) {
    .rest__row:hover {
      background: var(--hover);
    }
  }
  .rest__n {
    color: var(--ink-3);
    font-size: var(--text-md);
    line-height: var(--leading-md);
  }
  .rest__q {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    color: var(--ink-2);
    text-wrap: pretty;
  }
</style>
