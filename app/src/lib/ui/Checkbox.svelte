<!-- Checkbox: 16px, radius 4, accent when on. Toggling is instant apart from a 150ms color change. -->
<script lang="ts">
  interface Props {
    checked: boolean;
    /** What it includes, for assistive tech ("Include this rewrite"). */
    label: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
    class?: string;
  }

  let { checked = $bindable(), label, disabled = false, onchange, class: className }: Props = $props();
</script>

<input
  type="checkbox"
  class={["check", className]}
  bind:checked
  aria-label={label}
  {disabled}
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
  .check:disabled {
    border-color: var(--line-strong);
    background: var(--sunken);
  }
  .check:checked:disabled {
    background: var(--ink-4);
    border-color: var(--ink-4);
  }
</style>
