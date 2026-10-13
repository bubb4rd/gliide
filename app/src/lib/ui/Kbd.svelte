<!-- A key: inside buttons (the primary variant tints it), in the key footer, in menus. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  let { children }: { children: Snippet } = $props();
</script>

<kbd class="kbd">{@render children()}</kbd>

<style>
  .kbd {
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding-inline: var(--space-1);
    border-radius: var(--radius-xs);
    border: 1px solid transparent;
    font: var(--weight-medium) var(--text-xs) / 1 var(--font-ui);
    font-variant-numeric: tabular-nums;
    color: var(--ink-3);
    background: var(--sunken);
    box-shadow: var(--shadow-border);
  }
</style>
