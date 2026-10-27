<!--
  A round's prep on the Interview tab (DESIGN.md, Job → Interview): one model call, kept until what it was written
  from changes. What the round will likely test, the questions it will likely ask (grouped, each with why and whether
  you've practiced it; selecting one marks the lines it's about on the desk, ⏎ practices it), the requirements the
  resume you sent doesn't show with an honest answer for each, and questions to ask them.
-->
<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import Crosshair from "@lucide/svelte/icons/crosshair";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import type { PrepQuestion, PrepView, QuestionGroup, SentLine } from "$lib/api";
  import { Badge, Button, GroupHead, Icon, Thinking, tip } from "$lib/ui";

  interface Props {
    /** "the technical": the round, as the heading names it. */
    name: string;
    prep: PrepView | null;
    /** When the prep started being written (`performance.now()`); null while it isn't. */
    writing: number | null;
    failed: string | null;
    /** Question ids you've answered in practice. */
    practiced: Set<string>;
    lines: SentLine[];
    selected: string | null;
    onselect: (question: PrepQuestion) => void;
    onpractice: (question: PrepQuestion) => void;
    onagain: () => void;
    oncancel: () => void;
  }

  let {
    name,
    prep,
    writing,
    failed,
    practiced,
    lines,
    selected,
    onselect,
    onpractice,
    onagain,
    oncancel,
  }: Props = $props();

  const GROUPS: { value: QuestionGroup; label: string }[] = [
    { value: "resume", label: "About your resume" },
    { value: "skills", label: "Skills from the posting" },
    { value: "behavioral", label: "Behavioral" },
    { value: "logistics", label: "Logistics" },
  ];

  const byId = $derived(new Map(lines.map((l) => [l.id, l])));
  const groups = $derived(
    GROUPS.map((g) => ({ ...g, questions: prep?.questions.filter((q) => q.group === g.value) ?? [] })).filter(
      (g) => g.questions.length,
    ),
  );
  const done = $derived(prep?.questions.filter((q) => practiced.has(q.id)).length ?? 0);

  /** Where a line sits, for its tooltip: "Acme Widgets Co.: Wrote integration tests…". */
  const lineTip = (ids: string[]) =>
    ids
      .map((id) => byId.get(id))
      .filter((l) => l !== undefined)
      .map((l) => (l.entry ? `${l.entry}: ${l.text}` : l.text))
      .join("\n");
</script>

<section class="prep" aria-label="Prep for {name}">
  <div class="head">
    <GroupHead
      label="Prep for {name}"
      meta={prep ? `${prep.questions.length} questions · ${done} practiced` : ""}
    />
    {#if prep && writing === null}
      <Button variant="ghost" size="sm" icon={RotateCcw} onclick={onagain} {@attach tip("Write the prep again")}
        >Write again</Button
      >
    {/if}
  </div>

  {#if writing !== null}
    <div class="writing">
      <Thinking label="Writing prep from the posting and the resume you sent…" startedAt={writing} {oncancel} />
    </div>
  {:else if failed}
    <div class="failed">
      <p class="meta problem" role="alert">{failed}</p>
      <Button size="sm" icon={RotateCcw} onclick={onagain}>Try again</Button>
    </div>
  {:else if prep}
    <p class="focus"><Icon icon={Crosshair} size={16} />{prep.focus}</p>

    {#each groups as group (group.value)}
      <p class="label group">{group.label} <span class="num">{group.questions.length}</span></p>
      <ol class="questions">
        {#each group.questions as q (q.id)}
          <li>
            <button
              type="button"
              class={["question", q.id === selected && "question--selected"]}
              aria-current={q.id === selected || undefined}
              onclick={() => onselect(q)}
              ondblclick={() => onpractice(q)}
            >
              <span class="question__n num">{prep.questions.indexOf(q) + 1}</span>
              <span class="question__body">
                <span class="question__text">{q.text}</span>
                <span class="meta question__why"
                  >{#if q.lines.length}<span class="question__lines" {@attach tip(lineTip(q.lines))}
                      >{q.lines.length === 1 ? "1 line" : `${q.lines.length} lines`}</span
                    >{" · "}{/if}{q.why}</span
                >
              </span>
              {#if practiced.has(q.id)}
                <span class="meta practiced"><Icon icon={Check} size={14} />Practiced</span>
              {/if}
            </button>
          </li>
        {/each}
      </ol>
    {/each}

    {#if prep.gaps.length}
      <p class="label group">What the posting asks that you don’t show <span class="num">{prep.gaps.length}</span></p>
      <ul class="gaps">
        {#each prep.gaps as gap (gap.term)}
          <li class="gap">
            <p class="gap__term">{gap.term} <Badge>{gap.required ? "Required" : "Preferred"}</Badge></p>
            <blockquote class="gap__answer">{gap.answer}</blockquote>
            {#if gap.lines.length}
              <p class="meta" {@attach tip(lineTip(gap.lines))}>
                From {gap.lines.length === 1 ? "1 line" : `${gap.lines.length} lines`} you sent
              </p>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}

    {#if prep.askThem.length}
      <p class="label group">Ask them</p>
      <ul class="ask">
        {#each prep.askThem as ask (ask)}<li>{ask}</li>{/each}
      </ul>
    {/if}
  {/if}
</section>

<style>
  .prep {
    display: grid;
    padding: var(--space-2) var(--space-3) var(--space-6);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .head > :global(:first-child) {
    flex: 1;
    min-width: 0;
  }
  .writing,
  .failed {
    display: grid;
    justify-items: start;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
  }
  .problem {
    margin: 0;
    color: var(--danger);
  }
  .focus {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    margin: var(--space-1) 0 var(--space-2);
    padding: var(--space-2-5) var(--space-3);
    border-radius: var(--radius-md);
    background: var(--sunken);
    font-size: var(--text-md);
    line-height: var(--leading-md);
    text-wrap: pretty;
  }
  .focus :global(.icon) {
    flex: none;
    margin-top: 3px;
    color: var(--ink-3);
  }
  .group {
    margin: var(--space-4) var(--space-3) var(--space-1);
    color: var(--ink-3);
  }
  .questions,
  .gaps,
  .ask {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .question {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    font: inherit;
    color: inherit;
    text-align: start;
    cursor: default;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .question--selected {
    background: var(--selected);
  }
  .question:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .question__n {
    flex: none;
    width: 16px;
    color: var(--ink-3);
    font-size: var(--text-md);
    line-height: var(--leading-md);
  }
  .question__body {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: var(--space-0-5);
  }
  .question__text {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    text-wrap: pretty;
  }
  .question__lines {
    text-decoration: underline dotted var(--ink-4);
    text-underline-offset: 3px;
  }
  .practiced {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
  }
  .gap {
    display: grid;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3);
  }
  .gap p {
    margin: 0;
  }
  .gap__term {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-weight: var(--weight-medium);
  }
  .gap__answer {
    margin: 0;
    padding-inline-start: var(--space-3);
    box-shadow: inset 2px 0 0 var(--line-strong);
    font-size: var(--text-md);
    line-height: var(--leading-md);
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .ask li {
    padding: var(--space-1-5) var(--space-3);
    font-size: var(--text-md);
    line-height: var(--leading-md);
    text-wrap: pretty;
  }
  @media (hover: hover) {
    .question:not(.question--selected):hover {
      background: var(--hover);
    }
  }
</style>
