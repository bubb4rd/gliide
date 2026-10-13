<!--
  The head of a group of changes: the section label, then its count and points. Given `collapsed`, the whole head is
  a disclosure button (`aria-expanded`, `aria-controls` → `controls`): a click, Space or ⏎ folds the group away and
  brings it back. The chevron after the label points down while open and right while folded; the meta stays, so a
  folded group still says what it holds. No motion: folding is frequent.
-->
<script lang="ts">
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Icon from "./Icon.svelte";

  interface Props {
    label: string;
    meta?: string;
    /** Makes the head a toggle (bindable); omitted, it's a plain heading. */
    collapsed?: boolean;
    /** The id of the element the head shows and hides. */
    controls?: string;
    ontoggle?: (collapsed: boolean) => void;
  }

  let { label, meta, collapsed = $bindable(), controls, ontoggle }: Props = $props();

  function toggle() {
    collapsed = !collapsed;
    ontoggle?.(collapsed);
  }
</script>

{#if collapsed === undefined}
  <div class="group-head">
    <h3 class="label">{label}</h3>
    {#if meta}<span class="meta num">{meta}</span>{/if}
  </div>
{:else}
  <h3 class="heading">
    <button
      type="button"
      class={["group-head", "toggle", collapsed && "toggle--collapsed"]}
      aria-expanded={!collapsed}
      aria-controls={controls}
      onclick={toggle}
    >
      <span class="label">{label}</span>
      <Icon icon={collapsed ? ChevronRight : ChevronDown} size={12} weight="medium" class="chevron" />
      {#if meta}<span class="meta num">{meta}</span>{/if}
    </button>
  </h3>
{/if}

<style>
  .group-head {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-3) var(--space-1);
  }
  .meta {
    margin-inline-start: auto;
  }
  .heading {
    margin: 0;
    font: inherit;
  }
  /* The same box as the plain head, so a toggle lines up with one; 4px from the label to its chevron. */
  .toggle {
    width: 100%;
    gap: var(--space-1);
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    text-align: start;
    cursor: default;
    user-select: none;
  }
  .toggle .meta {
    padding-inline-start: var(--space-2);
  }
  .toggle :global(.chevron) {
    align-self: center;
    color: var(--ink-3);
    transition: color var(--dur-instant) var(--ease-out);
  }
  .toggle .label {
    transition: color var(--dur-instant) var(--ease-out);
  }
  @media (hover: hover) {
    .toggle:hover .label,
    .toggle:hover :global(.chevron) {
      color: var(--ink-2);
    }
  }
</style>
