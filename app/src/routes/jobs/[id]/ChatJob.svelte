<!--
  The job at the top of the chat's thread: the verdict and where (place, pay, source), the company's mark over the
  title, the posting's facts, the score (now › projected, ceiling, its parts on hover) and the keywords covered (on
  hover, each list: yours, added by your edits, missing in amber). Once it scrolls out of view, or you fold it, a
  one-line strip holds the top of the thread instead: the mark, the company and role, the score with a small bar and
  the keywords; its chevron brings the card back. The salary opens its panel (Add salary without one) and the mark the
  company photo's, both owned by Review.
-->
<script lang="ts">
  import Banknote from "@lucide/svelte/icons/banknote";
  import BriefcaseBusiness from "@lucide/svelte/icons/briefcase-business";
  import CalendarPlus from "@lucide/svelte/icons/calendar-plus";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import ChevronUp from "@lucide/svelte/icons/chevron-up";
  import ImagePlus from "@lucide/svelte/icons/image-plus";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { appError, type ChangeView, type ReviewView, type SalaryRange } from "$lib/api";
  import { logo } from "$lib/logo.svelte";
  import { salaryText } from "$lib/salary";
  import { Badge, Button, CompanyMark, Icon, ScoreHeader, tip, toast } from "$lib/ui";
  import { keywordLabel, keywordsOf, keywordTip } from "./keywords";
  import { signed } from "./place";

  interface Props {
    review: ReviewView;
    company: string;
    logoSrc: string | null;
    salary: SalaryRange | null;
    /** The thread's scroller: the card is out of view when it's left it. */
    root: HTMLElement | undefined;
    salaryAnchor?: HTMLButtonElement;
    logoMark?: HTMLElement;
    onsalary: () => unknown;
    canSalary: boolean;
  }

  let {
    review,
    company,
    logoSrc,
    salary,
    root,
    salaryAnchor = $bindable(),
    logoMark = $bindable(),
    onsalary,
    canSalary,
  }: Props = $props();

  const VERDICTS = {
    apply: { label: "Apply as is", tone: "accent" },
    tailor: { label: "Tailor first", tone: "accent" },
    stretch: { label: "Stretch", tone: "warn" },
    skip: { label: "Skip", tone: "neutral" },
  } as const;
  /** The strip's height: the card counts as out of view once it's gone under it. */
  const STRIP = 48;

  const head = $derived(review.head);
  const verdict = $derived(VERDICTS[review.verdict]);
  const now = $derived(Math.round(review.now.total));
  const projected = $derived(Math.round(review.projected.total));
  const counted = (c: ChangeView) => c.state === "in" || c.state === "forced";
  const edits = $derived(review.groups.flatMap((g) => g.changes).filter(counted).length);
  const pageNote = $derived.by(() => {
    const limit = review.pageLimit;
    if (review.pageCount <= limit) return `fits on ${limit} page${limit === 1 ? "" : "s"}`;
    return `${review.pageCount} pages: over by ${review.pageCount - limit}`;
  });
  const where = $derived(head.location);
  const added = $derived.by(() => {
    const at = new Date(review.job.addedAt);
    if (Number.isNaN(at.getTime())) return "";
    const days = Math.floor((Date.now() - at.getTime()) / 86_400_000);
    if (days <= 0) return "Added today";
    if (days === 1) return "Added yesterday";
    if (days < 30) return `Added ${days} days ago`;
    return `Added ${at.toLocaleDateString(undefined, { month: "short", day: "numeric" })}`;
  });

  // ── Keywords: the posting's terms on the page (yours, or added by an edit that's on), each list on hover ──
  const keywords = $derived(keywordsOf(review));
  const cells = $derived(
    Array.from({ length: keywords.of }, (_, i) =>
      i < keywords.have.length ? "have" : i < keywords.have.length + keywords.add.length ? "add" : "",
    ),
  );
  const covered = $derived(keywords.have.length + keywords.add.length);

  // ── Card or strip ──
  let card = $state<HTMLElement>();
  /** The card has scrolled out of view, under the strip. */
  let gone = $state(false);
  /** You folded the card: the strip takes its place. */
  let folded = $state(false);
  const strip = $derived(folded || gone);

  $effect(() => {
    if (!card || !root) return;
    const observer = new IntersectionObserver(([entry]) => (gone = !entry.isIntersecting), {
      root,
      rootMargin: `-${STRIP}px 0px 0px 0px`,
    });
    observer.observe(card);
    return () => {
      observer.disconnect();
      gone = false;
    };
  });

  function unfold() {
    folded = false;
    if (root) root.scrollTop = 0;
  }

  function openSource(e: MouseEvent) {
    e.preventDefault();
    if (head.url) openUrl(head.url).catch((err) => toast(appError(err).message, { tone: "danger" }));
  }
</script>

<!-- Holds the top of the thread: laid over it once the card has scrolled away, in its place once it's folded. -->
<div class={["pin", folded && "pin--folded"]}>
  {#if strip}
    <div class="strip">
      <CompanyMark size={20} src={logoSrc} />
      <span class="strip__company">{company}</span>
      <span class="strip__role" {@attach tip(head.role)}>{head.role}</span>
      <span class="strip__score num">
        <span class="strip__now">{now}%</span>
        <Icon icon={ChevronRight} size={12} />
        <span class="visually-hidden">to</span>
        <span class="strip__projected">{projected}%</span>
      </span>
      <span class="minibar" aria-hidden="true">
        <span class="minibar__now" style:width="{now}%"></span>
        {#if projected > now}<span class="minibar__gain" style:width="{projected - now}%"></span>{/if}
      </span>
      {#if keywords.of}
        <span class="strip__rule" aria-hidden="true"></span>
        <span
          class="meta num strip__keywords"
          role="img"
          aria-label={keywordLabel(keywords)}
          {@attach tip(() => keywordTip(keywords))}>{covered}/{keywords.of} keywords</span
        >
      {/if}
      <Button
        variant="ghost"
        size="sm"
        square
        icon={ChevronDown}
        aria-label="Show the job"
        {@attach tip("Show the job")}
        onclick={unfold}
      />
    </div>
  {/if}
</div>

{#if !folded}
  <header class="card" bind:this={card}>
    <div class="row">
      <Badge tone={verdict.tone}>{verdict.label}</Badge>
      <span class="meta row__meta num"
        >{where}{#if salary}{where ? " · " : ""}<button
            type="button"
            class="salary"
            {@attach tip("Change the salary range")}
            bind:this={salaryAnchor}
            onclick={onsalary}>{salaryText(salary)}</button
          >{/if}{#if head.source}{where || salary ? " · " : ""}{#if head.url}<a
              class="source"
              href={head.url}
              onclick={openSource}>{head.source}</a
            >{:else}{head.source}{/if}{/if}</span
      >
      {#if canSalary && !salary}
        <Button bind:element={salaryAnchor} variant="ghost" size="sm" icon={Banknote} onclick={onsalary}
          >Add salary</Button
        >
      {/if}
      {#if head.companyKey && !logoSrc}
        <Button variant="ghost" size="sm" icon={ImagePlus} popovertarget="logo-panel">Upload company photo</Button>
      {/if}
      <Button
        variant="ghost"
        size="sm"
        square
        icon={ChevronUp}
        aria-label="Fold the job"
        {@attach tip("Fold the job")}
        onclick={() => (folded = true)}
      />
    </div>

    <div class={["named", !head.company && "named--solo"]}>
      <span class="lead">
        {#if head.companyKey}
          <CompanyMark
            size={48}
            src={logoSrc}
            label={logoSrc ? `Change ${head.company}'s photo` : "Upload company photo"}
            over={logo.over === head.company}
            bind:element={logoMark}
            popovertarget="logo-panel"
            data-logo-drop={head.company}
          />
        {:else}
          <CompanyMark size={48} />
        {/if}
      </span>
      {#if head.company}<p class="company" {@attach tip(head.company)}>{head.company}</p>{/if}
      <h1 class="headline">{head.role}</h1>
    </div>

    {#if head.seniority || added}
      <ul class="facts meta">
        {#if head.seniority}<li><Icon icon={BriefcaseBusiness} size={14} />{head.seniority}</li>{/if}
        {#if added}<li><Icon icon={CalendarPlus} size={14} />{added}</li>{/if}
      </ul>
    {/if}

    <ScoreHeader
      now={review.now.total}
      projected={review.projected.total}
      ceiling={review.ceiling.total}
      components={review.projected.components}
      nowTitle="Your {review.master.title} master as it is"
    >
      <strong>{signed(projected - now)} points</strong> from {edits}
      {edits === 1 ? "edit" : "edits"} · {pageNote}
    </ScoreHeader>

    {#if keywords.of}
      <div class="keywords" role="img" aria-label={keywordLabel(keywords)} {@attach tip(() => keywordTip(keywords))}>
        <span class="meter">
          {#each cells as cell, i (i)}<span class={["cell", cell && `cell--${cell}`]}></span>{/each}
        </span>
        <span class="keywords__count"><strong class="num">{covered} of {keywords.of}</strong> keywords</span>
      </div>
    {/if}
  </header>
{/if}

<style>
  /* Sticks to the thread's top; takes no room while the card is there, so nothing moves when the strip shows. */
  .pin {
    position: sticky;
    top: 0;
    z-index: 2;
    height: 0;
  }
  .pin--folded {
    height: 48px;
  }
  .strip {
    display: flex;
    align-items: center;
    gap: var(--space-2-5);
    height: 48px;
    padding-inline: var(--space-6) var(--space-3);
    background: var(--paper);
    box-shadow: inset 0 -1px 0 var(--line);
    white-space: nowrap;
  }
  .strip__company {
    flex: none;
    font-weight: var(--weight-medium);
  }
  .strip__role {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--ink-2);
  }
  .strip__score {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
  }
  .strip__now {
    color: var(--ink-3);
  }
  .strip__score :global(.icon) {
    color: var(--ink-4);
  }
  .strip__projected {
    font-weight: var(--weight-medium);
  }
  /* now | gain, as the score's bar in small (its edge as the bar's: a ring over a transparent border). */
  .minibar {
    display: flex;
    flex: none;
    gap: 1px;
    width: 56px;
    height: 6px;
    overflow: hidden;
    border-radius: var(--radius-full);
    border: 1px solid transparent;
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .minibar__now {
    border-radius: var(--radius-full);
    background: var(--ink-3);
  }
  .minibar__gain {
    border-radius: var(--radius-full);
    background: var(--gain);
  }
  .strip__rule {
    flex: none;
    width: 1px;
    height: 16px;
    background: var(--line-strong);
  }
  .strip__keywords {
    flex: none;
  }

  /* The card: the thread's first block, on paper, over a hairline. */
  .card {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-4);
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--line);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .row__meta {
    flex: 1;
    min-width: 0;
    color: var(--ink-2);
  }
  .source {
    color: var(--accent-text);
    text-decoration: none;
  }
  /* Reads as part of the line; a click opens the salary panel. */
  .salary {
    padding: 0;
    border: 0;
    border-radius: var(--radius-xs);
    background: none;
    font: inherit;
    color: inherit;
    cursor: default;
  }
  @media (hover: hover) {
    .source:hover,
    .salary:hover {
      text-decoration: underline;
    }
    .salary:hover {
      color: var(--ink);
    }
  }
  /*
   * As the letter tab's header: the 48px mark spans the company and the title's first line, its edges on the company's cap
   * height and the title's baseline. Without a company it centers on the title's first line.
   */
  .named {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: var(--space-3);
    row-gap: var(--space-0-5);
    align-items: start;
    margin-top: calc(var(--space-1) * -1);
  }
  .lead {
    display: flex;
    grid-row: span 2;
    margin-top: 6px;
  }
  .named--solo .lead {
    grid-row: auto;
    margin-top: calc((var(--leading-headline) - 48px) / 2);
  }
  .company {
    min-width: 0;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-lg);
    line-height: var(--leading-lg);
    letter-spacing: var(--tracking-lg);
    font-weight: var(--weight-medium);
    color: var(--ink-2);
  }
  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1-5) var(--space-4);
    margin: calc(var(--space-1) * -1) 0 0;
    padding: 0;
    list-style: none;
  }
  .facts li {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
  }
  .facts :global(.icon) {
    color: var(--ink-3);
  }

  /*
   * n of m: a cell a keyword, yours in gain, added by an edit in gain's wash; as wide as it draws, so its tooltip (each
   * list) opens only over the meter and the count.
   */
  .keywords {
    display: flex;
    justify-self: start;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
    max-width: 100%;
    white-space: nowrap;
    cursor: default;
  }
  /* A long list never widens the line: the cells narrow (to 3px) before the count gives way. */
  .meter {
    display: flex;
    flex: 0 1 auto;
    min-width: 0;
    gap: 2px;
  }
  .cell {
    flex: 0 1 auto;
    width: 8px;
    min-width: 3px;
    height: 8px;
    border-radius: 2px;
    border: 1px solid var(--line-strong);
    background: var(--sunken);
  }
  .cell--have {
    border-color: transparent;
    background: var(--gain);
  }
  .cell--add {
    border-color: var(--gain);
    background: var(--gain-soft);
  }
  .keywords__count {
    flex: none;
    color: var(--ink-2);
  }
  .keywords__count strong {
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
</style>
