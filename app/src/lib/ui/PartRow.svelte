<!--
  One part of a document you edit in place (a cover letter's greeting, a paragraph, the closing): its name, its text,
  and actions that show on hover, focus or selection. Like a change row without the checkbox: nothing to toggle, only
  to edit. `busy` dims the text while a model call rewrites it. Selection and hover are instant (no motion).
-->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    /** What the part is ("Greeting", "Paragraph 2"). */
    label: string;
    selected?: boolean;
    /** Being rewritten: the text stays, dimmed, until the new one arrives. */
    busy?: boolean;
    onselect?: () => void;
    text: Snippet;
    actions?: Snippet;
    /** Replaces the text while editing. */
    editor?: Snippet;
  }

  let { label, selected = false, busy = false, onselect, text, actions, editor }: Props = $props();

  let row: HTMLDivElement;

  // Keep the selected row in view as selection moves; a row that mounts selected leaves the scroll alone.
  let moved = false;
  $effect(() => {
    if (selected && moved) row.scrollIntoView({ block: "nearest" });
    moved = true;
  });
</script>

<!-- Rows are selected from the keyboard by the screen's ↑↓ binding; focusing anything inside selects it too. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
  bind:this={row}
  class={["part", busy && "part--busy"]}
  data-selected={selected || undefined}
  aria-current={selected || undefined}
  aria-busy={busy || undefined}
  onclick={() => onselect?.()}
  onfocusin={() => onselect?.()}
>
  <div class="part__head">
    <span class="part__label">{label}</span>
    {#if actions && !editor}<div class="part__actions">{@render actions()}</div>{/if}
  </div>
  <div class="part__text">
    {#if editor}{@render editor()}{:else}{@render text()}{/if}
  </div>
</div>

<style>
  .part {
    display: grid;
    row-gap: var(--space-1);
    padding: var(--space-2) var(--space-3) var(--space-3);
    border-radius: var(--radius-md);
    border: 1px solid transparent;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  /* As a change row's foot: when the label and the actions don't fit their full width on one line, the actions wrap
     under the label instead of squeezing it. */
  .part__head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
    min-height: var(--control-sm);
  }
  .part__label {
    flex: 1 1 auto;
    min-width: 0;
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    font-weight: var(--weight-medium);
    color: var(--ink-3);
  }
  .part__actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-0-5);
    margin-inline-start: auto;
    opacity: 0;
  }
  .part:focus-within .part__actions,
  .part[data-selected] .part__actions {
    opacity: 1;
  }
  .part__text {
    font-size: var(--text-md);
    line-height: var(--leading-md);
    letter-spacing: var(--tracking-md);
    color: var(--ink);
    text-wrap: pretty;
  }
  .part--busy .part__text {
    color: var(--ink-3);
  }
  .part[data-selected] {
    background: var(--selected);
  }

  @media (hover: hover) {
    .part:hover {
      background: var(--hover);
    }
    .part[data-selected]:hover {
      background: var(--selected);
    }
    .part:hover .part__actions {
      opacity: 1;
    }
  }
</style>
