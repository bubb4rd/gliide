<!-- The sticky key footer: the screen's keys and what they do. -->
<script lang="ts">
  import Kbd from "./Kbd.svelte";

  interface Props {
    keys: { keys: string[]; label: string }[];
  }

  let { keys }: Props = $props();
</script>

<footer class="keys" aria-label="Keyboard shortcuts">
  {#each keys as item (item.label)}
    <span class="item">
      {#each item.keys as key, i (i)}<Kbd>{key}</Kbd>{/each}
      {item.label}
    </span>
  {/each}
</footer>

<style>
  .keys {
    position: sticky;
    bottom: 0;
    z-index: 1;
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1) var(--space-4);
    padding: var(--space-3) var(--space-6);
    background: var(--paper);
    box-shadow: inset 0 1px 0 var(--line);
    color: var(--ink-3);
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
    user-select: none;
  }
  .item {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
  }
  .item :global(.kbd + .kbd) {
    margin-inline-start: calc(var(--space-1) * -1);
  }
</style>
