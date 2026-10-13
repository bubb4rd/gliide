<!-- Segmented control: one choice of a few. Radius 8 = 6 inner + 1 padding + 1 border. -->
<script lang="ts" generics="T">
  interface Props {
    options: { value: T; label: string }[];
    value: T;
    /** What the choice is, for assistive tech ("Suggestions from"). */
    label: string;
    disabled?: boolean;
    onchange?: (value: T) => void;
  }

  let { options, value = $bindable(), label, disabled = false, onchange }: Props = $props();

  function choose(next: T) {
    if (next === value) return;
    value = next;
    onchange?.(next);
  }
</script>

<div class="seg" role="group" aria-label={label}>
  {#each options as option (option.value)}
    <button type="button" aria-pressed={option.value === value} {disabled} onclick={() => choose(option.value)}>
      {option.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    flex: none;
    gap: var(--space-0-5);
    padding: 1px;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  button {
    height: var(--control-sm);
    padding-inline: var(--space-2-5);
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    font: var(--weight-medium) var(--text-sm) / 1 var(--font-ui);
    color: var(--ink-3);
    background: transparent;
    cursor: default;
    user-select: none;
    transition-property: color, background-color;
    transition-duration: var(--dur-instant);
    transition-timing-function: var(--ease-out);
  }
  button[aria-pressed="true"] {
    color: var(--ink);
    background: var(--surface);
    box-shadow: var(--shadow-border);
  }
  button:disabled {
    color: var(--ink-4);
  }
  button[aria-pressed="true"]:disabled {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--line);
  }
  @media (hover: hover) {
    button[aria-pressed="false"]:not(:disabled):hover {
      color: var(--ink-2);
    }
  }
</style>
