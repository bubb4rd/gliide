<!--
  A chip you press. As a posting's skill: have: on the resume now (gain). add: the plan adds the posting's wording
  (dashed gain). missing: not on the resume. not-mine: you said it isn't yours (struck). `p1`: the posting's
  priority-1 skills. As a plain toggle (a story's themes): on, the selection wash with accent text and a check, never
  green; off, neutral. Pressing toggles it; the state is also its color and icon, so nothing relies on motion.
-->
<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import Icon from "./Icon.svelte";

  interface Props extends HTMLButtonAttributes {
    state: "have" | "add" | "missing" | "not-mine" | "on" | "off";
    p1?: boolean;
    children: Snippet;
  }

  let { state, p1 = false, children, class: className, ...rest }: Props = $props();
</script>

<button
  type="button"
  class={["chip", `chip--${state}`, p1 && "chip--p1", className]}
  aria-pressed={state === "have" || state === "add" || state === "on"}
  {...rest}
>
  {#if state === "have" || state === "on"}<Icon icon={Check} size={12} weight="medium" />{/if}
  {#if state === "add"}<Icon icon={Plus} size={12} weight="medium" />{/if}
  {@render children()}
</button>

<style>
  .chip {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    height: var(--control-sm);
    padding-inline: var(--space-2);
    border-radius: var(--radius-full);
    border: 1px solid transparent;
    font: var(--weight-medium) var(--text-sm) / 1 var(--font-ui);
    white-space: nowrap;
    color: var(--ink-2);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
    cursor: default;
    user-select: none;
    transition-property: scale, background-color, color;
    transition-duration: var(--dur-fast);
    transition-timing-function: var(--ease-out);
  }
  /* Optical alignment: the icon side sits 2px tighter. */
  .chip--have,
  .chip--add,
  .chip--on {
    padding-inline-start: calc(var(--space-2) - 2px);
  }
  .chip--p1 {
    font-weight: var(--weight-semibold);
  }
  @media (prefers-reduced-motion: no-preference) {
    .chip:not(:disabled):active {
      scale: var(--press-scale);
    }
  }
  /* On the resume now */
  .chip--have {
    color: var(--gain-text);
    background: var(--gain-soft);
    box-shadow: none;
  }
  /* You have it; the plan adds the posting's wording */
  .chip--add {
    color: var(--gain-text);
    background: transparent;
    box-shadow: none;
    border: 1px dashed oklch(from var(--gain) l c h / 0.6);
  }
  .chip--missing {
    color: var(--ink-3);
  }
  /* Marked not yours */
  .chip--not-mine {
    color: var(--ink-4);
    text-decoration: line-through;
    box-shadow: none;
    background: transparent;
  }
  /* A toggle that's on: selected, not a gain */
  .chip--on {
    color: var(--accent-text);
    background: var(--selected);
    box-shadow: none;
  }
  .chip:disabled {
    opacity: 0.6;
  }

  @media (hover: hover) {
    .chip:not(:disabled):hover {
      background: var(--hover);
    }
    .chip--have:not(:disabled):hover {
      background: oklch(from var(--gain-soft) calc(l - 0.02) c h);
    }
    .chip--on:not(:disabled):hover {
      background: oklch(from var(--selected) l c h / calc(alpha + 0.04));
    }
  }
</style>
