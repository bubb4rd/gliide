<!-- Badge: verdicts, page status, application status. Color carries meaning (DESIGN.md Honesty in color). -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    tone?: "neutral" | "accent" | "gain" | "warn" | "danger";
    /** A status dot before the text. */
    dot?: boolean;
    children: Snippet;
  }

  let { tone = "neutral", dot = false, children }: Props = $props();
</script>

<span class={["badge", `badge--${tone}`]}>
  {#if dot}<span class="dot" aria-hidden="true"></span>{/if}
  {@render children()}
</span>

<style>
  .badge {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    height: 20px;
    padding-inline: var(--space-2);
    border-radius: var(--radius-full);
    border: 1px solid transparent;
    font: var(--weight-medium) var(--text-xs) / 1 var(--font-ui);
    letter-spacing: 0.01em;
    white-space: nowrap;
    color: var(--ink-2);
    background: var(--sunken);
  }
  .badge--neutral {
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .badge--accent {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .badge--gain {
    color: var(--gain-text);
    background: var(--gain-soft);
  }
  .badge--warn {
    color: var(--warn);
    background: var(--warn-soft);
  }
  .badge--danger {
    color: var(--danger);
    background: var(--danger-soft);
  }
  /* Optical alignment: the dot side sits 2px tighter. */
  .badge:has(.dot) {
    padding-inline-start: calc(var(--space-2) - 2px);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: var(--radius-full);
    background: currentColor;
  }
</style>
