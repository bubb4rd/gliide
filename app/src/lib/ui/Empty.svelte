<!-- Empty state: serif title, one sentence, one primary action with its key. Enters with `.enter`, staggered. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    body: string;
    action?: Snippet;
  }

  let { title, body, action }: Props = $props();
</script>

<div class="empty">
  <h2 class="title enter" style:--i="0">{title}</h2>
  <p class="enter" style:--i="1">{body}</p>
  {#if action}<div class="action enter" style:--i="2">{@render action()}</div>{/if}
</div>

<style>
  .empty {
    display: grid;
    justify-items: center;
    gap: var(--space-2);
    padding: var(--space-12) var(--space-6);
    text-align: center;
  }
  p {
    margin: 0;
    max-width: 36ch;
    color: var(--ink-2);
    text-wrap: pretty;
  }
  .action {
    margin-top: var(--space-3);
  }
</style>
