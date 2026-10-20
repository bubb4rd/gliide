<!--
  A tooltip's panel (DESIGN.md Tooltip): a label, or data (a title with its value, labeled rows with legend keys, named
  lists, a note). Screens don't place it: `tip()` gives an element one and the layout's <Tooltips /> shows it. The
  specimen renders it in place.
-->
<script lang="ts">
  import type { HTMLAttributes } from "svelte/elements";
  import type { TipContent } from "./tip.svelte";

  interface Props extends HTMLAttributes<HTMLDivElement> {
    content: TipContent;
  }

  let { content, class: className, ...rest }: Props = $props();

  const data = $derived(typeof content === "string" ? null : content);
  const hasHead = $derived(data !== null && (data.title != null || data.value != null));
</script>

<div class={["tooltip", data && "tooltip--data", className]} {...rest}>
  {#if data === null}
    <p class="text">{content}</p>
  {:else}
    {#if hasHead}
      <p class="head">
        {#if data.title != null}<span class="name">{data.title}</span>{/if}
        {#if data.value != null}<span class={["value", "num", data.tone && `tone--${data.tone}`]}>{data.value}</span>{/if}
      </p>
    {/if}
    {#if data.rows?.length}
      <dl class="rows">
        {#each data.rows as row, i (i)}
          <div class="row">
            <dt>
              {#if row.key}<span class="key key--{row.key}" aria-hidden="true"></span>{/if}
              {row.label}
            </dt>
            <dd class={["num", row.tone && `tone--${row.tone}`]}>{row.value ?? ""}</dd>
          </div>
        {/each}
      </dl>
    {/if}
    {#if data.groups?.length}
      <div class="groups">
        {#each data.groups as group, i (i)}
          <div class="group">
            <p class="group__head">
              {#if group.key}<span class="key key--{group.key}" aria-hidden="true"></span>{/if}
              <span class="group__label">{group.label}</span>
              <span class="num group__count">{group.items.length}</span>
            </p>
            <p class={["group__items", group.tone && `tone--${group.tone}`]}>{group.items.join(", ")}</p>
          </div>
        {/each}
      </div>
    {/if}
    {#if data.note}<p class="note">{data.note}</p>{/if}
  {/if}
</div>

<style>
  .tooltip {
    display: grid;
    gap: var(--space-1);
    width: max-content;
    max-width: min(280px, calc(100vw - 2 * var(--space-2)));
    padding: var(--space-1) var(--space-2);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    letter-spacing: var(--tracking-sm);
    color: var(--ink);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
  }
  .tooltip--data {
    padding: var(--space-2) var(--space-3);
  }
  p,
  dl,
  dd {
    margin: 0;
  }
  .text,
  .note {
    text-wrap: pretty;
    /* Paths and links have no spaces to wrap at. */
    overflow-wrap: anywhere;
  }
  .note {
    color: var(--ink-3);
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-4);
    font-weight: var(--weight-medium);
  }
  .rows {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--space-0-5) var(--space-4);
  }
  .row {
    display: contents;
  }
  dt {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    min-width: 0;
    color: var(--ink-2);
  }
  dd {
    text-align: end;
  }
  .tone--gain {
    color: var(--gain-text);
  }
  .tone--warn {
    color: var(--warn);
  }

  /* Legend keys draw the mark they stand for: the score bar's segments (pills) and its ceiling tick. */
  .key {
    flex: none;
    width: 12px;
    height: 8px;
    border-radius: var(--radius-full);
  }
  .key--base {
    background: var(--ink-3);
  }
  .key--gain {
    background: var(--gain);
  }
  .key--lost {
    background: var(--warn-soft);
    box-shadow: inset 0 0 0 1px var(--warn);
  }
  .key--tick {
    width: 2px;
    height: 12px;
    margin-inline: 5px;
    background: var(--ink-2);
  }
  /* The keyword meter's cells, as it draws them: filled gain, gain's wash in a gain edge, an empty well. */
  .key--have,
  .key--add,
  .key--missing {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .key--have {
    background: var(--gain);
  }
  .key--add {
    background: var(--gain-soft);
    box-shadow: inset 0 0 0 1px var(--gain);
  }
  .key--missing {
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line-strong);
  }

  /* Named lists: a head (key, label in ink 2, the count in ink 3), the items wrapping under it. */
  .groups {
    display: grid;
    gap: var(--space-2);
  }
  .group__head {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    color: var(--ink-2);
  }
  .group__count {
    margin-inline-start: auto;
    color: var(--ink-3);
  }
  .group__items {
    margin-top: var(--space-0-5);
    text-wrap: pretty;
  }

  @media (forced-colors: active) {
    .key {
      forced-color-adjust: none;
      background: CanvasText;
    }
    .key--gain,
    .key--have {
      background: Highlight;
    }
    .key--lost,
    .key--add,
    .key--missing {
      background: Canvas;
      border: 1px solid CanvasText;
    }
    .key--add {
      border-color: Highlight;
    }
  }
</style>
