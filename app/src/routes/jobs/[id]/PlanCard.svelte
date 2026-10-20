<!--
  The chat's opening plan: the master's suggested edits, read live from the review, one numbered line each (what it
  does and where, its points, a mark for its state: on, off, or left out, why in the triangle's tooltip). A line opens
  in place to the edit itself (the line it replaces over the line as it will read) with Turn on, Turn off or Keep
  anyway, and a triangle whose tooltip says what to check. The foot turns on every edit that's off.
-->
<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import CheckCheck from "@lucide/svelte/icons/check-check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import X from "@lucide/svelte/icons/x";
  import type { ChangeView, ReviewView } from "$lib/api";
  import { Button, Icon, Marked, tip } from "$lib/ui";
  import { signed, type Place } from "./place";
  import type { Session } from "./session.svelte";

  interface Item {
    change: ChangeView;
    place: Place;
  }

  interface Props {
    s: Session;
    review: ReviewView;
    items: Item[];
    /** Commands can run (the review isn't being replaced). */
    ready: boolean;
  }

  let { s, review, items, ready }: Props = $props();

  const id = $props.id();
  const counted = (c: ChangeView) => c.state === "in" || c.state === "forced";
  /** One line for what the edit does: Suggest's reason; a summary without one by what it does (its text is long). */
  function describe(c: ChangeView): string {
    if (c.why) return c.why;
    if (c.op === "summary") return c.was ? "Rewrite the summary" : "Add a summary";
    return c.text;
  }
  const offKeys = $derived(items.filter((x) => x.change.state === "off").map((x) => x.change.key));
  const now = $derived(Math.round(review.now.total));
  const projected = $derived(Math.round(review.projected.total));

  let open = $state(new Set<string>());

  function flip(key: string) {
    const next = new Set(open);
    if (!next.delete(key)) next.add(key);
    open = next;
  }
</script>

<section class="plan" aria-labelledby="{id}-title">
  <header class="head">
    <h3 class="title" id="{id}-title">Plan</h3>
    <span class="meta">{items.length} {items.length === 1 ? "edit" : "edits"}</span>
    <span class="meta num proj"
      >{now}% <span aria-hidden="true">→</span><span class="visually-hidden">to</span>
      <strong>{projected}%</strong></span
    >
  </header>

  <ol class="list">
    {#each items as { change, place }, i (change.key)}
      {@const expanded = open.has(change.key)}
      {@const points = change.points == null ? null : Math.round(change.points)}
      <li class={["item", `item--${change.state}`, expanded && "item--open"]}>
        <button
          type="button"
          class="row"
          aria-expanded={expanded}
          aria-controls="{id}-edit-{i}"
          onclick={() => flip(change.key)}
        >
          <span class="n num" aria-hidden="true">{i + 1}</span>
          <span class="what">
            <span class="line">
              <span class="desc">{describe(change)}</span>
              {#if place.main !== "Summary" || place.sub}
                <span class="where meta">{place.main}{place.sub ? ` › ${place.sub}` : ""}</span>
              {/if}
            </span>
          </span>
          <span class={["pts", "num", (points === null || !counted(change) || points <= 0) && "pts--quiet"]}
            >{points === null ? "" : signed(points)}</span
          >
          <span class="mark">
            {#if counted(change)}
              <Icon icon={Check} size={14} weight="bold" /><span class="visually-hidden">On</span>
            {:else if change.state === "left-out"}
              <span class="warning" {@attach tip(change.reason ? `Left out: ${change.reason}` : "Left out")}
                ><Icon icon={TriangleAlert} size={14} weight="medium" /><span class="visually-hidden"
                  >Left out{change.reason ? `: ${change.reason}` : ""}</span
                ></span
              >
            {:else}
              <span class="ring"></span><span class="visually-hidden">Off</span>
            {/if}
          </span>
          <span class="chev"><Icon icon={expanded ? ChevronDown : ChevronRight} size={14} /></span>
        </button>

        {#if expanded}
          <div class="edit" id="{id}-edit-{i}">
            {#if change.was}
              <p class="diff diff--old"><span class="sign" aria-hidden="true">−</span><span class="text">{change.was}</span></p>
            {/if}
            <p class="diff"><span class="sign" aria-hidden="true">+</span><span><Marked text={change.text} marks={change.marks} /></span></p>
            {#if change.meta}<p class="meta detail">{change.meta}</p>{/if}
            <div class="actions">
              {#if counted(change)}
                <Button variant="ghost" size="sm" icon={X} disabled={!ready} onclick={() => s.toggle(change.key)}
                  >Turn off</Button
                >
              {:else if change.state === "off"}
                <Button size="sm" icon={Check} disabled={!ready} onclick={() => s.toggle(change.key)}>Turn on</Button>
              {:else}
                <Button size="sm" disabled={!ready} onclick={() => s.toggle(change.key)}>Keep anyway</Button>
              {/if}
              {#if change.yours}<span class="meta owner">Your edit</span>{/if}
              {#if change.warning}
                <span class="warning" {@attach tip(change.warning)}
                  ><Icon icon={TriangleAlert} size={14} weight="medium" /><span class="visually-hidden"
                    >{change.warning}</span
                  ></span
                >
              {/if}
            </div>
          </div>
        {/if}
      </li>
    {/each}
  </ol>

  <footer class="foot">
    {#if offKeys.length}
      <Button size="sm" icon={CheckCheck} disabled={!ready} onclick={() => s.accept(offKeys)}>Turn on all</Button>
    {/if}
    <span class="meta hint">Each edit shows on the page as it turns on; ⌘Z takes it back</span>
  </footer>
</section>

<style>
  /* As a Card: surface, ring and lift; the rows inside it, the foot on paper under a hairline. */
  .plan {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-width: 0;
    overflow: hidden;
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-border);
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    padding: var(--space-2-5) var(--space-3);
    border-bottom: 1px solid var(--line);
  }
  .title {
    margin: 0;
    font: inherit;
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .proj {
    margin-inline-start: auto;
    white-space: nowrap;
  }
  .proj strong {
    font-weight: var(--weight-medium);
    color: var(--gain-text);
  }

  /* 8 inner + 3 inset + 1 border = the card's 12. */
  .list {
    display: grid;
    margin: 0;
    padding: calc(var(--space-1) - 1px);
    list-style: none;
  }
  .row {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) auto 16px 14px;
    align-items: start;
    column-gap: var(--space-2-5);
    width: 100%;
    padding: var(--space-1-5) var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    font: inherit;
    text-align: start;
    color: var(--ink);
    cursor: default;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .row:focus-visible {
    outline: none;
    box-shadow: var(--focus-ring);
  }
  .item--open .row {
    background: var(--hover);
  }
  @media (hover: hover) {
    .row:hover {
      background: var(--hover);
    }
  }
  .n {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    margin-top: 1px;
    border-radius: var(--radius-full);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    color: var(--ink-2);
    border: 1px solid var(--line-strong);
    background: var(--sunken);
  }
  .what {
    display: grid;
    min-width: 0;
    line-height: 22px;
  }
  /* One line: what it does, then where (meta), cut at the column's edge. */
  .line {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .desc {
    color: var(--ink);
  }
  .where {
    margin-inline-start: var(--space-1-5);
  }
  .item--off .desc,
  .item--left-out .desc {
    color: var(--ink-2);
  }
  /* A warning is its triangle alone; what to check is in its tooltip (and read out). */
  .warning {
    display: inline-flex;
    color: var(--warn);
  }
  .pts {
    font-size: var(--text-sm);
    line-height: 22px;
    font-weight: var(--weight-medium);
    color: var(--gain-text);
  }
  .pts--quiet {
    color: var(--ink-3);
  }
  /* The state, on the line's center: a check (on), an empty ring (off), a triangle (left out). */
  .mark {
    display: grid;
    place-items: center;
    height: 22px;
    color: var(--gain-text);
  }
  .item--left-out .mark {
    color: var(--warn);
  }
  .ring {
    width: 12px;
    height: 12px;
    border-radius: var(--radius-full);
    border: 1.5px solid var(--line-strong);
  }
  .chev {
    display: grid;
    place-items: center;
    height: 22px;
    color: var(--ink-3);
  }

  /*
   * The edit itself, under its line: the − and + signs (14px, then 6px) sit left of the row's text, so the lines
   * start where the description does (the row's 8px inset, the 20px number, the 10px gap).
   */
  .edit {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--space-1-5);
    padding: var(--space-2) var(--space-2) var(--space-3)
      calc(var(--space-2) + 20px + var(--space-2-5) - 14px - var(--space-1-5));
  }
  .diff {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr);
    gap: var(--space-1-5);
    margin: 0;
    font-size: var(--text-md);
    line-height: var(--leading-md);
    letter-spacing: var(--tracking-md);
    text-wrap: pretty;
    overflow-wrap: anywhere;
    color: var(--ink);
  }
  .sign {
    font-weight: var(--weight-medium);
    text-align: center;
    color: var(--ink-4);
  }
  .diff--old {
    color: var(--ink-3);
  }
  .diff--old .text {
    text-decoration-line: line-through;
    text-decoration-color: var(--ink-4);
  }
  .detail {
    margin: 0;
    padding-inline-start: calc(14px + var(--space-1-5));
    text-wrap: pretty;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding-inline-start: calc(14px + var(--space-1-5) - var(--space-2));
    margin-top: var(--space-1);
  }
  .owner {
    color: var(--accent-text);
  }

  .foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-top: 1px solid var(--line);
    background: var(--paper);
  }
  .hint {
    margin-inline-start: auto;
    text-align: end;
  }
</style>
