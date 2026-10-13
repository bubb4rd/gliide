<!-- Card: a surface with the shadow ring. `pad` gives it the standard 20px inset. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  interface Props extends HTMLAttributes<HTMLElement> {
    pad?: boolean;
    /** `section` when it has its own heading. */
    as?: "div" | "section";
    children: Snippet;
  }

  let { pad = false, as = "div", children, class: className, ...rest }: Props = $props();
</script>

<svelte:element this={as} class={["card", pad && "card--pad", className]} {...rest}>
  {@render children()}
</svelte:element>

<style>
  .card {
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-border);
  }
  .card--pad {
    padding: var(--space-5);
  }
</style>
