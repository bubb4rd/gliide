<!-- Sidebar navigation: a list of NavItems. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    /** Names the landmark when a window has more than one nav ("Main", "Recent jobs"). */
    label?: string;
    children: Snippet;
  }

  let { label, children }: Props = $props();
</script>

<nav class="nav" aria-label={label}>{@render children()}</nav>

<style>
  .nav {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 1px;
  }
</style>
