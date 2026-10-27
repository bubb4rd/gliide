<!--
  Typed practice for one round (DESIGN.md, Job → Interview → Practice): the interviewer asks the prep's questions one
  at a time; you answer in the box at the foot; each answer comes back (one model call) as a feedback card: the lines
  it drew on, which of situation, task, action and result it has, what it says differently from the resume you sent
  (amber), what it says the bank doesn't hold yet (Keep as fact), a tighter telling and the likely follow-up. Never a
  grade. The strip at the top pins the question, with how many you've practiced. `N` next question, `R` again, `Esc`
  back to the prep.
-->
<script lang="ts">
  import { tick } from "svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import Check from "@lucide/svelte/icons/check";
  import CircleDashed from "@lucide/svelte/icons/circle-dashed";
  import Copy from "@lucide/svelte/icons/copy";
  import Plus from "@lucide/svelte/icons/plus";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import SkipForward from "@lucide/svelte/icons/skip-forward";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import UserRound from "@lucide/svelte/icons/user-round";
  import {
    appError,
    cancel,
    keepClaim,
    practice,
    type NewClaim,
    type PracticeTurn,
    type PrepQuestion,
    type PrepView,
    type Round,
    type SentLine,
    type StarPart,
  } from "$lib/api";
  import { bindKeys } from "$lib/keys";
  import { kindLabel } from "$lib/rounds";
  import { Button, Icon, KeyFooter, type KeyCommand, Textarea, Thinking, tip, toast } from "$lib/ui";

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
  let thread = $state<HTMLElement>();
  // Practice starts in the answer box.
  $effect(() => {
    field?.focus();
  });

  async function send() {
    const text = answer.trim();
    if (!text || !asked || reading !== null) return;
    reading = performance.now();
    try {
      const turn = await practice(round.id, asked.id, text);
      turns = [...turns, turn];
      answer = "";
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

  async function ask(q: PrepQuestion | undefined, same = false) {
    if (!q || reading !== null) return;
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

  async function copy(text: string) {
    try {
      await writeText(text);
      toast("Copied");
    } catch (e) {
      toast(`Couldn’t copy: ${appError(e).message}`, { tone: "danger" });
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

  const STAR: { key: "situation" | "task" | "action" | "result"; label: string }[] = [
    { key: "situation", label: "Situation" },
    { key: "task", label: "Task" },
    { key: "action", label: "Action" },
    { key: "result", label: "Result" },
  ];
  const partWord = (p: StarPart) => (p === "yes" ? "" : p === "thin" ? " thin" : " missing");
  const clip = (text: string) => (text.length > 44 ? `${text.slice(0, 43).trimEnd()}…` : text);

  const interactive = (target: EventTarget | null) =>
    target instanceof HTMLElement && target.closest("button, a, input, textarea, select") !== null;

  const KEYS: KeyCommand[] = $derived([
    { keys: ["⏎"], label: "Send your answer", run: () => void send(), disabled: !answer.trim() || reading !== null },
    { keys: ["N"], label: "Next question", run: () => void ask(next), disabled: reading !== null },
    { keys: ["R"], label: "Ask it again", run: () => void ask(asked, true), disabled: reading !== null },
    { keys: ["Esc"], label: "Back to prep", run: onback },
    ...keys,
  ]);

  $effect(() =>
    bindKeys({
      n: (e) => (interactive(e.target) ? false : void ask(next)),
      r: (e) => (interactive(e.target) ? false : void ask(asked, true)),
      Escape: (e) => {
        if (reading !== null) return void cancel(job);
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
    {@const f = turn.feedback}
    <article class="card" aria-label="Feedback on your answer">
      {#if f.lines.length}
        <div class="row">
          <span class="row__label">Drew on</span>
          <div class="chips">
            {#each f.lines as id (id)}
              {@const line = byId.get(id)}
              {#if line}
                <span class="chip" {@attach tip(line.entry ? `${line.entry}: ${line.text}` : line.text)}
                  >{clip(line.text)}</span
                >
              {/if}
            {/each}
          </div>
        </div>
      {/if}
      <div class="row">
        <span class="row__label">STAR</span>
        <div>
          <p class="star">
            {#each STAR as part (part.key)}
              <span class={["star__part", f[part.key] !== "yes" && "star__part--short"]}
                ><Icon icon={f[part.key] === "yes" ? Check : CircleDashed} size={14} weight="medium" />{part.label}{partWord(
                  f[part.key],
                )}</span
              >
            {/each}
          </p>
          {#if f.starNote}<p class="meta">{f.starNote}</p>{/if}
        </div>
      </div>
      {#each f.mismatches as m (m.said + m.line)}
        <div class="row row--warn">
          <span class="row__label">Consistency</span>
          <p class="warn">
            <Icon icon={TriangleAlert} size={16} weight="medium" /><span
              >You said {m.said}; the resume you sent says {m.resume}.</span
            >
          </p>
        </div>
      {/each}
      {#if f.newClaims.length}
        <div class="row">
          <span class="row__label">New to gliide</span>
          <div class="claims">
            {#each f.newClaims as claim (claim.text)}
              <div class="claim">
                <p>“{claim.text}”</p>
                {#if kept.has(claim.text)}
                  <p class="meta kept"><Icon icon={Check} size={14} />Kept as a fact of {claim.entryName}</p>
                {:else}
                  <p class="claim__keep">
                    <Button size="sm" icon={Plus} onclick={() => keep(claim)}>Keep as fact</Button>
                    <span class="meta">On {claim.entryName}, your answer as its source</span>
                  </p>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/if}
      {#if f.tighter}
        {@const tighter = f.tighter}
        <div class="row">
          <span class="row__label">Tighter</span>
          <div>
            <p class="tighter">{tighter}</p>
            <p class="claim__keep">
              <Button variant="ghost" size="sm" icon={Copy} onclick={() => copy(tighter)}>Copy</Button>
              <span class="meta">Only your answer and what you sent</span>
            </p>
          </div>
        </div>
      {/if}
      {#if f.followUp}
        <div class="row">
          <span class="row__label">Follow-up</span>
          <p class="follow">“{f.followUp}”</p>
        </div>
      {/if}
    </article>
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
  {#if reading !== null}
    <div class="reading">
      <Thinking label="Reading your answer…" startedAt={reading} oncancel={() => void cancel(job)} />
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
    <Button variant="ghost" size="sm" icon={RotateCcw} kbd="R" disabled={reading !== null} onclick={() => ask(asked, true)}
      >Again</Button
    >
    <Button
      variant={answered ? "primary" : "default"}
      size="sm"
      icon={SkipForward}
      kbd="N"
      disabled={reading !== null}
      onclick={() => ask(next)}>Next question</Button
    >
  </div>
  <div class="composer">
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
        disabled={!answer.trim() || reading !== null}
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
  .card {
    display: grid;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-border);
    overflow: hidden;
  }
  .row {
    display: grid;
    grid-template-columns: 84px minmax(0, 1fr);
    gap: var(--space-3);
    padding: var(--space-2-5) var(--space-3);
  }
  .row + .row {
    box-shadow: inset 0 1px 0 var(--line);
  }
  .row p {
    margin: 0;
  }
  .row__label {
    padding-top: 2px;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-3);
  }
  .row--warn {
    background: var(--warn-soft);
  }
  .warn {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    font-weight: var(--weight-medium);
  }
  .warn :global(.icon) {
    flex: none;
    margin-top: 2px;
    color: var(--warn);
  }
  .chips,
  .star {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1-5) var(--space-3);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    height: 22px;
    padding-inline: var(--space-2);
    border-radius: var(--radius-full);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
    font-size: var(--text-sm);
    color: var(--ink-2);
  }
  .star__part {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .star__part :global(.icon) {
    color: var(--gain-text);
  }
  .star__part--short {
    color: var(--ink-3);
  }
  .star__part--short :global(.icon) {
    color: var(--ink-3);
  }
  .claims {
    display: grid;
    gap: var(--space-3);
  }
  .claim {
    display: grid;
    gap: var(--space-1-5);
  }
  .row .claim__keep {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-1-5);
  }
  .kept {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }
  .kept :global(.icon) {
    color: var(--gain-text);
  }
  .tighter,
  .follow {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    text-wrap: pretty;
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
