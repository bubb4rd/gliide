<!--
  The company's logo slot: the logo you set (ARCHITECTURE.md §7, Company logos), else a blank tile. A logo shows as
  the company publishes it, scaled to fit the square and rounded like the tile: never recolored, filtered or outlined.
  Sizes: 16 (one-line sidebar items), 20 (one-line table cells, beside a name), 28 (two lines: the sidebar's Recent
  jobs, the Applications table and the Overview's Up next, from the company's cap height to the role's baseline), 48
  (the job header: from the company's cap height to the title's baseline). With children, the company's name follows
  the mark and truncates. With `label` (its accessible name) it's a button that sets the logo (`onclick`, or
  `popovertarget` for a panel): the blank tile shows an image icon on hover and focus, and `over` (files dragged over
  it) rings it in the accent.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import ImagePlus from "@lucide/svelte/icons/image-plus";
  import Icon from "./Icon.svelte";

  interface Props extends Omit<HTMLButtonAttributes, "children"> {
    size?: 16 | 20 | 28 | 48;
    /** The logo's URL (`app.logos`); the blank tile without one. */
    src?: string | null;
    /** Files are dragged over the button: it's where they'd drop. */
    over?: boolean;
    /** Makes the mark a button, named this. */
    label?: string;
    element?: HTMLElement;
    /** The company's name beside the mark. */
    children?: Snippet;
  }

  let {
    size = 20,
    src = null,
    over = false,
    label,
    element = $bindable(),
    children,
    class: className,
    ...rest
  }: Props = $props();
</script>

{#snippet mark()}
  {#if label}
    <button
      bind:this={element}
      type="button"
      class={["mark", `mark--${size}`, "mark--button", src && "mark--logo", over && "mark--over", className]}
      aria-label={label}
      {...rest}
    >
      {#if src}
        <img {src} alt="" draggable="false" />
      {:else}
        <span class="add"><Icon icon={ImagePlus} size={size >= 28 ? 16 : 12} /></span>
      {/if}
    </button>
  {:else if src}
    <img bind:this={element} class={["mark", `mark--${size}`, "mark--logo", className]} {src} alt="" aria-hidden="true" draggable="false" />
  {:else}
    <span bind:this={element} class={["mark", `mark--${size}`, className]} aria-hidden="true"></span>
  {/if}
{/snippet}

{#if children}
  <span class="company">
    {@render mark()}
    <span class="name">{@render children()}</span>
  </span>
{:else}
  {@render mark()}
{/if}

<style>
  .mark {
    display: block;
    flex: none;
    box-sizing: border-box;
    margin: 0;
    padding: 0;
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
  .mark--48 {
    width: 48px;
    height: 48px;
    border-radius: var(--radius-md);
  }
  /* The company's own artwork, rounded like the tile: no tile color or ring behind or around it. */
  .mark--logo {
    overflow: hidden;
    border: 0;
    background: none;
    box-shadow: none;
    object-fit: contain;
  }
  .mark--logo img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .mark--button {
    display: grid;
    place-items: center;
    color: var(--ink-3);
    cursor: default;
    -webkit-tap-highlight-color: transparent;
    transition-property: scale, box-shadow, background-color;
    transition-duration: var(--dur-fast);
    transition-timing-function: var(--ease-out);
  }
  @media (prefers-reduced-motion: no-preference) {
    .mark--button:not(:disabled):active {
      scale: var(--press-scale);
    }
  }
  .add {
    display: grid;
    opacity: 0;
    transition-property: opacity;
    transition-duration: var(--dur-instant);
    transition-timing-function: var(--ease-out);
  }
  .mark--button:focus-visible .add,
  .mark--over .add {
    opacity: 1;
  }
  .mark--over {
    color: var(--accent-text);
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  /* An opaque logo covers an inset ring: ring it outside instead. */
  .mark--over.mark--logo {
    box-shadow: 0 0 0 2px var(--accent);
  }
  @media (hover: hover) {
    .mark--button:not(.mark--logo):not(.mark--over):hover {
      color: var(--ink-2);
      background: var(--hover);
      box-shadow: inset 0 0 0 1px var(--input-border);
    }
    .mark--button:hover .add {
      opacity: 1;
    }
  }
  @media (hover: none) {
    .add {
      opacity: 1;
    }
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
