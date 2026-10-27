<!--
  Checkbox: 16px, radius 4, accent when on. Toggling is instant apart from a 150ms color change. `mixed` (some of a
  set on: a table's select-all) draws a bar instead of the check and reads as mixed to assistive tech.
-->
<script lang="ts">
  interface Props {
    checked: boolean;
    /** What it includes, for assistive tech ("Include this rewrite"). */
    label: string;
    /** Some, not all, of what it stands for is on. */
    mixed?: boolean;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
    /** Fires before `onchange`, with the click's modifier keys (a table's Shift-click range). */
    onclick?: (e: MouseEvent) => void;
    class?: string;
  }

  let {
    checked = $bindable(),
    label,
    mixed = false,
    disabled = false,
    onchange,
    onclick,
    class: className,
  }: Props = $props();
</script>

<input
  type="checkbox"
  class={["check", className]}
  bind:checked
  {@attach (el) => {
    el.indeterminate = mixed;
  }}
  aria-label={label}
  {disabled}
  {onclick}
  onchange={() => onchange?.(checked)}
/>

<style>
  .check {
    appearance: none;
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 16px;
    height: 16px;
    margin: 0;
    border-radius: var(--radius-xs);
    border: 1px solid var(--input-border);
    background: var(--surface);
    transition-property: background-color, border-color;
    transition-duration: var(--dur-fast);
    transition-timing-function: var(--ease-out);
  }
  .check:checked {
    background: var(--accent);
    border-color: var(--accent);
  }
  .check:checked::after {
    content: "";
    width: 9px;
    height: 5px;
    border-inline-start: 2px solid var(--on-accent);
    border-block-end: 2px solid var(--on-accent);
    rotate: -45deg;
    translate: 0 -1px;
  }
  .check:indeterminate {
    background: var(--accent);
    border-color: var(--accent);
  }
  .check:indeterminate::after {
    content: "";
    width: 8px;
    border-block-end: 2px solid var(--on-accent);
  }
  .check:disabled {
    border-color: var(--line-strong);
    background: var(--sunken);
  }
  .check:checked:disabled {
    background: var(--ink-4);
    border-color: var(--ink-4);
  }
</style>
