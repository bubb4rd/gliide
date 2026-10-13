<!--
  The company's logo slot: blank until logos are fetched (ARCHITECTURE.md §7, Company logos). A fetched logo will show
  in this slot as the company publishes it: no recolor, filter, crop or outline.
  Sizes: 16 (one-line sidebar items), 20 (tables), 28 (the sidebar's two-line Recent jobs: from the company's cap
  height to the role's baseline), 32 (the job header). With children, the company's name follows the mark and
  truncates.
-->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    size?: 16 | 20 | 28 | 32;
    /** The company's name beside the mark. */
    children?: Snippet;
  }

  let { size = 20, children }: Props = $props();
</script>

{#if children}
  <span class="company">
    <span class={["mark", `mark--${size}`]} aria-hidden="true"></span>
    <span class="name">{@render children()}</span>
  </span>
{:else}
  <span class={["mark", `mark--${size}`]} aria-hidden="true"></span>
{/if}

<style>
  .mark {
    display: block;
    flex: none;
    box-sizing: border-box;
    border: 1px solid transparent;
    border-radius: var(--radius-xs);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line-strong);
  }
  .mark--16 {
    width: 16px;
    height: 16px;
  }
  .mark--20 {
    width: 20px;
    height: 20px;
  }
  .mark--28 {
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
  }
  .mark--32 {
    width: 32px;
    height: 32px;
    border-radius: var(--radius-sm);
  }
  .company {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
