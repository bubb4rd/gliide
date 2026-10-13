<!--
  One change, toggled and edited in place. States: in, off (you turned it off), left-out (the fit check or validator
  left it out, with why), forced (left out, kept on by you; still says why). Actions show on hover, focus or
  selection; toggles are instant (no motion) and every state is carried by color and text.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import Checkbox from "./Checkbox.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    state: "in" | "off" | "left-out" | "forced";
    checked: boolean;
    selected?: boolean;
    /** Marginal points; null when it can't be toggled alone. */
    points?: number | null;
    /** The checkbox's name for assistive tech ("Include this rewrite"). */
    label: string;
    /** May return a promise (the command); the box then shows the review's state again once it settles. */
    ontoggle: (checked: boolean) => unknown;
    onselect?: () => void;
    text: Snippet;
    /** The line it replaces. */
    was?: string | null;
    /** Why it was left out. */
    reason?: string | null;
    /** Kept although it runs long. */
    warning?: string | null;
    /** You wrote it ("Your edit"). */
    yours?: boolean;
    meta?: string;
    actions?: Snippet;
    /** Replaces the text while editing. */
    editor?: Snippet;
  }

  let {
    state: rowState,
    checked,
    selected = false,
    points = null,
    label,
    ontoggle,
    onselect,
    text,
    was,
    reason,
    warning,
    yours = false,
    meta,
    actions,
    editor,
  }: Props = $props();

  let row: HTMLDivElement;
  // The box follows the review; a toggle shows at once and settles back to whatever the command returned.
  let box = $state(false);
  $effect.pre(() => {
    box = checked;
  });

  async function toggle(next: boolean) {
    try {
      await ontoggle(next);
    } finally {
      box = checked;
    }
  }

  // Keep the selected row in view as selection moves; a row that mounts selected leaves the scroll alone.
  let moved = false;
  $effect(() => {
    if (selected && moved) row.scrollIntoView({ block: "nearest" });
    moved = true;
  });

  const pts = $derived(
    points == null ? null : points > 0 ? `+${Math.round(points)}` : points < 0 ? `−${Math.abs(Math.round(points))}` : "0",
  );
  const reasonText = $derived(
    reason ? (rowState === "forced" ? `Kept on by you · ${reason}` : rowState === "left-out" ? `Left out: ${reason}` : reason) : null,
  );
</script>

<!-- Rows are selected from the keyboard by the screen's ↑↓ binding; focusing anything inside selects it too. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
  bind:this={row}
  class={["change", `change--${rowState}`]}
  data-selected={selected || undefined}
  aria-current={selected || undefined}
  onclick={() => onselect?.()}
  onfocusin={() => onselect?.()}
>
  <Checkbox class="change__check" bind:checked={box} {label} onchange={toggle} />
  <div class="change__text">
    {#if editor}{@render editor()}{:else}{@render text()}{/if}
  </div>
  {#if pts !== null}<span class="change__pts num">{pts}</span>{/if}
  {#if was && !editor}<div class="change__was" title={was}>Was “{was}”</div>{/if}
  {#if reasonText || warning}
    <div class="change__notes">
      {#if reasonText}
        <span class="change__reason"><Icon icon={TriangleAlert} size={14} /><span>{reasonText}</span></span>
      {/if}
      {#if warning}
        <span class="change__reason"><Icon icon={TriangleAlert} size={14} /><span>{warning}</span></span>
      {/if}
    </div>
  {/if}
  {#if yours || meta || actions}
    <div class="change__foot">
      {#if yours}<span class="change__owner">Your edit</span>{/if}
      {#if meta}<span class="meta change__meta">{meta}</span>{/if}
      {#if actions}<div class="change__actions">{@render actions()}</div>{/if}
    </div>
  {/if}
</div>

<style>
  .change {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    column-gap: var(--space-3);
    row-gap: var(--space-1);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    border: 1px solid transparent;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .change :global(.change__check) {
    margin-top: 3px;
  }
  .change__text {
    grid-column: 2;
    font-size: var(--text-md);
    line-height: var(--leading-md);
    letter-spacing: var(--tracking-md);
    color: var(--ink);
    text-wrap: pretty;
  }
  .change__was {
    grid-column: 2;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .change__pts {
    grid-column: 3;
    grid-row: 1;
    font: var(--weight-medium) var(--text-sm) / var(--leading-md) var(--font-ui);
    font-variant-numeric: tabular-nums;
    color: var(--gain-text);
  }
  /* Reasons get their own lines, full width: a long one never squeezes into the gap beside the actions. */
  .change__notes {
    grid-column: 2 / -1;
    display: grid;
    gap: var(--space-1);
  }
  /* One line: meta wraps inside itself, so hidden actions never push a blank line. */
  .change__foot {
    grid-column: 2 / -1;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--control-sm);
  }
  .change__meta {
    flex: 1 1 0;
    min-width: 0;
  }
  .change__actions {
    display: flex;
    flex: none;
    gap: var(--space-0-5);
    margin-inline-start: auto;
    opacity: 0;
  }
  .change:focus-within .change__actions,
  .change[data-selected] .change__actions {
    opacity: 1;
  }
  .change[data-selected] {
    background: var(--selected);
  }
  .change--off .change__text,
  .change--left-out .change__text {
    color: var(--ink-3);
  }
  .change--off .change__pts,
  .change--left-out .change__pts {
    color: var(--ink-4);
  }
  .change__reason {
    display: inline-flex;
    align-items: flex-start;
    gap: var(--space-1);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    color: var(--warn);
  }
  .change__reason :global(.icon) {
    margin-top: 1px;
  }
  .change--forced .change__reason {
    color: var(--ink-3);
  }
  .change__owner {
    flex: none;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    font-weight: var(--weight-medium);
    color: var(--accent-text);
  }

  @media (hover: hover) {
    .change:hover {
      background: var(--hover);
    }
    .change[data-selected]:hover {
      background: var(--selected);
    }
    .change:hover .change__actions {
      opacity: 1;
    }
  }
</style>
