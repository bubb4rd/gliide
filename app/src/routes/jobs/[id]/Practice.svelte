<!--
  Typed practice for one round (DESIGN.md, Job → Interview → Practice): the interviewer asks the prep's questions one
  at a time; you answer in the box at the foot; each answer comes back (one model call) as a feedback card
  (`FeedbackCard`): the lines it drew on, which of situation, task, action and result it has, what it says differently
  from the resume you sent (amber), what it says the bank doesn't hold yet (Keep as fact), a tighter telling and the
  likely follow-up. Never a grade. An answer that tells a story offers Save as story (`S`). Stuck, **Nudge** (`⌘J`, one
  call) reads your draft and your last answer to the question for a few small suggestions (`NudgeCard`); Add to answer
  puts one's example at the end of your draft, its first `…` selected. The strip at the top pins the question, with
  how many you've practiced. `N` next question, `R` again, `Esc` back to the prep.
-->
<script lang="ts">
  import { tick } from "svelte";
  import { goto } from "$app/navigation";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import BookOpen from "@lucide/svelte/icons/book-open";
  import Check from "@lucide/svelte/icons/check";
  import Lightbulb from "@lucide/svelte/icons/lightbulb";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import SkipForward from "@lucide/svelte/icons/skip-forward";
  import UserRound from "@lucide/svelte/icons/user-round";
  import {
    appError,
    cancel,
    keepClaim,
    nudge,
    practice,
    saveStory,
    type NewClaim,
    type Nudge,
    type PracticeTurn,
    type PrepQuestion,
    type PrepView,
    type Round,
    type SentLine,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { kindLabel } from "$lib/rounds";
  import { Button, Icon, KeyFooter, type KeyCommand, Textarea, Thinking, tip, toast } from "$lib/ui";
  import FeedbackCard from "./FeedbackCard.svelte";
  import NudgeCard from "./NudgeCard.svelte";

  interface Props {
    job: string;
    company: string;
    round: Round;
    prep: PrepView;
    lines: SentLine[];
    /** The round's practiced answers, oldest first (bindable: a new one is added here). */
    turns: PracticeTurn[];
    /** The question being asked (bindable). */
    question: string;
    /** The screen's keys, after the practice's own in Commands. */
    keys: KeyCommand[];
    onback: () => void;
  }

  let { job, company, round, prep, lines, turns = $bindable(), question = $bindable(), keys, onback }: Props =
    $props();

  const byId = $derived(new Map(lines.map((l) => [l.id, l])));
  const asked = $derived(prep.questions.find((q) => q.id === question) ?? prep.questions[0]);
  const practiced = $derived(new Set(turns.map((t) => t.question)));
  const order = $derived(prep.questions.findIndex((q) => q.id === asked?.id));
  /** The first question after this one you haven't practiced, else the next in order. */
  const next = $derived.by(() => {
    const after = [...prep.questions.slice(order + 1), ...prep.questions.slice(0, order)];
    return after.find((q) => !practiced.has(q.id)) ?? after[0];
  });
  /** Who asks: the round's interviewer by first name, else the company. */
  const asker = $derived(round.with ? `${round.with.split(/[ ,]/)[0]}, as ${company}` : `${company}’s interviewer`);
  let answer = $state("");
  let reading = $state<number | null>(null);
  /** You asked for the same question again: it waits for a new answer though the last turn answered it. */
  let again = $state(false);
  /** The last turn answers the question being asked: then it's answered, and the next one waits for N. */
  const answered = $derived(turns.at(-1)?.question === asked?.id && !again);
  let kept = $state(new Set<string>());
  let field = $state<Textarea>();
  let composer = $state<HTMLElement>();
  let thread = $state<HTMLElement>();
  /** The nudges shown, and the question they're for. */
  let nudges = $state<{ question: string; items: Nudge[] } | null>(null);
  let nudging = $state<number | null>(null);
  /** A call is running: answers and nudges wait for it. */
  const busy = $derived(reading !== null || nudging !== null);
  const shown = $derived(nudges && nudges.question === asked?.id ? nudges.items : null);
  // Practice starts in the answer box.
  $effect(() => {
    field?.focus();
  });

  async function send() {
    const text = answer.trim();
    if (!text || !asked || busy) return;
    reading = performance.now();
    try {
      const turn = await practice(round.id, asked.id, text);
      turns = [...turns, turn];
      answer = "";
      nudges = null;
      again = false;
      (document.activeElement as HTMLElement | null)?.blur();
      await tick();
      thread?.scrollTo({ top: thread.scrollHeight });
    } catch (e) {
      const error = appError(e);
      if (error.kind !== "cancelled") toast(error.message, { tone: "danger" });
    } finally {
      reading = null;
    }
  }

  /** One call: nudges for the question being asked, read with what you've typed so far. */
  async function askNudge() {
    if (!asked || busy) return;
    const q = asked.id;
    nudging = performance.now();
    try {
      const items = await nudge(round.id, q, answer);
      nudges = { question: q, items };
      await tick();
      thread?.scrollTo({ top: thread.scrollHeight });
    } catch (e) {
      const error = appError(e);
      if (error.kind !== "cancelled") toast(error.message, { tone: "danger" });
    } finally {
      nudging = null;
    }
  }

  /** Puts a nudge's example at the end of your draft and selects its first `…`, where your words go. */
  async function addToAnswer(example: string) {
    const base = answer.trimEnd();
    answer = base ? `${base} ${example}` : example;
    await tick();
    const box = composer?.querySelector("textarea");
    if (!box) return;
    box.focus();
    const gap = answer.indexOf("…", base.length);
    if (gap >= 0) box.setSelectionRange(gap, gap + 1);
    else box.setSelectionRange(answer.length, answer.length);
  }

  async function ask(q: PrepQuestion | undefined, same = false) {
    if (!q || busy) return;
    question = q.id;
    again = same;
    answer = "";
    await tick();
    thread?.scrollTo({ top: thread.scrollHeight });
    field?.focus();
  }

  async function keep(claim: NewClaim) {
    try {
      await keepClaim(round.id, claim.entry, claim.text);
      kept = new Set([...kept, claim.text]);
      toast(`Kept as a fact of ${claim.entryName}`);
    } catch (e) {
      toast(appError(e).message, { tone: "danger" });
    }
  }

  /** Turns saved as a story this visit: the turn's id → the story's. */
  let saved = $state(new Map<string, string>());
  /** The newest turn with a story to save that isn't saved yet: what `S` saves. */
  const saveable = $derived(turns.findLast((t) => t.feedback.story && !saved.has(t.id)));
  let saving = $state(false);

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

  function onFieldKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    } else if (e.key === "Escape") {
      e.stopPropagation();
      (e.currentTarget as HTMLElement).blur();
    }
  }

  const interactive = (target: EventTarget | null) =>
    target instanceof HTMLElement && target.closest("button, a, input, textarea, select") !== null;

  const KEYS: KeyCommand[] = $derived([
    { keys: ["⏎"], label: "Send your answer", run: () => void send(), disabled: !answer.trim() || busy },
    { keys: ["⌘J"], label: "Nudge", run: () => void askNudge(), disabled: busy },
    { keys: ["N"], label: "Next question", run: () => void ask(next), disabled: busy },
    { keys: ["R"], label: "Ask it again", run: () => void ask(asked, true), disabled: busy },
    ...(saveable ? [{ keys: ["S"], label: "Save as story", run: () => void saveAsStory(saveable), disabled: saving }] : []),
    { keys: ["Esc"], label: "Back to prep", run: onback },
    ...keys,
  ]);

  $effect(() =>
    bindKeys({
      n: (e) => (interactive(e.target) ? false : void ask(next)),
      r: (e) => (interactive(e.target) ? false : void ask(asked, true)),
      s: (e) => (interactive(e.target) || !saveable ? false : void saveAsStory(saveable)),
      "mod+j": () => void askNudge(),
      Escape: (e) => {
        if (busy) return void cancel(job);
        if (interactive(e.target)) return false;
        onback();
      },
    }),
  );
</script>

<div class="pin">
  <div class="strip">
    <Icon icon={UserRound} size={16} />
    <span class="strip__n">Question {order + 1}</span>
    <span class="strip__q" {@attach tip(asked?.text ?? "")}>{asked?.text}</span>
    <span class="meter" aria-hidden="true">
      {#each prep.questions as q (q.id)}
        <span class={["cell", practiced.has(q.id) && "cell--done", q.id === asked?.id && "cell--now"]}></span>
      {/each}
    </span>
    <span class="meta num">{practiced.size} of {prep.questions.length} practiced</span>
  </div>
</div>

<div class="thread" bind:this={thread}>
  {#each turns as turn (turn.id)}
    <article class="turn">
      <p class="who"><Icon icon={UserRound} size={16} /><span class="who__name">{asker}</span></p>
      <p class="asked">{turn.asked}</p>
    </article>
    <p class="bubble">{turn.answer}</p>
    {#snippet story()}
      {@const id = saved.get(turn.id)}
      {#if id}
        <a class="meta saved" href={storyHref(id)}><Icon icon={Check} size={14} />Saved as a story</a>
      {:else}
        <Button
          variant="ghost"
          size="sm"
          icon={BookOpen}
          kbd={turn === saveable ? "S" : undefined}
          disabled={saving}
          onclick={() => saveAsStory(turn)}>Save as story</Button
        >
      {/if}
    {/snippet}
    <FeedbackCard
      feedback={turn.feedback}
      line={(id) => byId.get(id)}
      {kept}
      onkeep={keep}
      grounded="Only your answer and what you sent"
      actions={turn.feedback.story ? story : undefined}
    />
  {/each}

  {#if asked && !answered}
    <article class="turn">
      <p class="who">
        <Icon icon={UserRound} size={16} /><span class="who__name">{asker}</span><span class="meta"
          >Question {order + 1} of {prep.questions.length} · {kindLabel(round.kind)}</span
        >
      </p>
      <p class="asked">{asked.text}</p>
    </article>
  {/if}
  {#if shown}
    <NudgeCard
      nudges={shown}
      line={(id) => byId.get(id)}
      onadd={(example) => void addToAnswer(example)}
      ondismiss={() => (nudges = null)}
    />
  {/if}
  {#if reading !== null}
    <div class="reading">
      <Thinking label="Reading your answer…" startedAt={reading} oncancel={() => void cancel(job)} />
    </div>
  {:else if nudging !== null}
    <div class="reading">
      <Thinking label="Nudging…" startedAt={nudging} oncancel={() => void cancel(job)} />
    </div>
  {/if}
</div>

<div class="foot">
  <div class="foot__row">
    {#if answered && next}
      <span class="meta next">Next: “{next.text}”</span>
    {:else}
      <span class="meta next">Answer as you would out loud</span>
    {/if}
    <Button variant="ghost" size="sm" icon={Lightbulb} kbd="⌘J" disabled={busy || !asked} onclick={() => askNudge()}
      >Nudge</Button
    >
    <Button variant="ghost" size="sm" icon={RotateCcw} kbd="R" disabled={busy} onclick={() => ask(asked, true)}
      >Again</Button
    >
    <Button
      variant={answered ? "primary" : "default"}
      size="sm"
      icon={SkipForward}
      kbd="N"
      disabled={busy}
      onclick={() => ask(next)}>Next question</Button
    >
  </div>
  <div class="composer" bind:this={composer}>
    <Textarea
      bind:this={field}
      bind:value={answer}
      placeholder={answered ? "Answer again, or go on with N" : "Your answer…"}
      rows={3}
      aria-label="Your answer"
      disabled={reading !== null}
      onkeydown={onFieldKey}
    />
    <div class="composer__bar">
      <KeyFooter keys={KEYS} bare />
      <Button
        variant="primary"
        size="sm"
        square
        icon={ArrowUp}
        aria-label="Send your answer"
        {@attach tip("Send your answer (⏎)")}
        disabled={!answer.trim() || busy}
        onclick={() => void send()}
      />
    </div>
  </div>
</div>

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
  }
  .strip :global(.icon) {
    flex: none;
    color: var(--ink-3);
  }
  .strip__n {
    flex: none;
    font-weight: var(--weight-medium);
  }
  .strip__q {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-2);
  }
  /* Practicing isn't points: the meter is ink, the question being asked outlined in the accent. */
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
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-5) var(--space-6);
  }
  /* The thread scrolls; its turns and cards keep their height. A card clips its corners (`overflow: hidden`), which
     lets a flex item shrink below its content, so without this the cards would squash instead of the thread scrolling. */
  .thread > :global(*) {
    flex: none;
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
  .asked {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    font-weight: var(--weight-medium);
    text-wrap: pretty;
  }
  .bubble {
    align-self: flex-end;
    max-width: 88%;
    margin: 0;
    padding: var(--space-2-5) var(--space-3);
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: var(--text-md);
    line-height: var(--leading-md);
    white-space: pre-wrap;
    text-wrap: pretty;
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
  .reading {
    padding-inline: var(--space-1);
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
  }
  .next {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
</style>
