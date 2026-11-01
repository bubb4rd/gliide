<!--
  Switch: something on or off as a whole (an agent in Settings), where a Checkbox is one item of a list. 32×20, radius
  full, a 14px knob 2px in: off, a sunken track on the input edge with the knob in ink 3; on, the accent track with
  the knob in on-accent. Toggling is instant apart from a 150ms color change: the knob jumps, as a checkbox's check.
  `locked` can't be changed but stays focusable (`aria-disabled`), so a tooltip on it can say why: locked on, a soft
  accent track with an accent knob; locked off, the knob in ink 4.
-->
<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";

  interface Props extends Omit<HTMLButtonAttributes, "onchange" | "disabled"> {
    checked: boolean;
    /** What it turns on, for assistive tech ("Codex"). */
    label: string;
    /** Can't be changed now; the reason belongs in a tooltip on it. */
    locked?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let { checked = $bindable(), label, locked = false, onchange, class: className, ...rest }: Props = $props();

  function toggle() {
    if (locked) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  {...rest}
  type="button"
  role="switch"
  class={["switch", className]}
  aria-checked={checked}
  aria-label={label}
  aria-disabled={locked || undefined}
  onclick={toggle}
>
  <span class="knob"></span>
</button>

<style>
  .switch {
    display: inline-flex;
    align-items: center;
    flex: none;
    width: 32px;
    height: 20px;
    margin: 0;
    padding: 1px;
    border: 1px solid var(--input-border);
    border-radius: var(--radius-full);
    background: var(--sunken);
    transition-property: background-color, border-color;
    transition-duration: var(--dur-fast);
    transition-timing-function: var(--ease-out);
  }
  .knob {
    width: 14px;
    height: 14px;
    border-radius: var(--radius-full);
    background: var(--ink-3);
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .switch[aria-checked="true"] {
    border-color: var(--accent);
    background: var(--accent);
  }
  .switch[aria-checked="true"] .knob {
    margin-inline-start: auto;
    background: var(--on-accent);
  }
  .switch[aria-disabled="true"] {
    border-color: var(--line-strong);
  }
  .switch[aria-disabled="true"] .knob {
    background: var(--ink-4);
  }
  .switch[aria-checked="true"][aria-disabled="true"] {
    border-color: var(--accent-soft);
    background: var(--accent-soft);
  }
  .switch[aria-checked="true"][aria-disabled="true"] .knob {
    background: var(--accent);
  }
  @media (forced-colors: active) {
    .switch[aria-checked="true"] .knob {
      background: Highlight;
    }
  }
</style>
