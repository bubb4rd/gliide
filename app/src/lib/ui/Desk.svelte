<!--
  The desk: the resume pages (live SVG) on a darker surface, the only object with real depth. The bar holds the
  page-status badge, optional meta, and zoom. 100% fits the page to the desk (at most US Letter's 612px).
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import Minus from "@lucide/svelte/icons/minus";
  import Plus from "@lucide/svelte/icons/plus";
  import Button from "./Button.svelte";

  interface Props {
    /** One SVG document per page, from our renderer. */
    pages: string[];
    /** The page-status badge. */
    status: Snippet;
    /** Percent, 50–200. */
    zoom?: number;
    /** Between the badge and zoom ("Highlights show what changed"). */
    children?: Snippet;
  }

  let { pages, status, zoom = $bindable(100), children }: Props = $props();

  const STEPS = [50, 67, 75, 90, 100, 110, 125, 150, 175, 200];
  const zoomOut = $derived(STEPS.findLast((s) => s < zoom));
  const zoomIn = $derived(STEPS.find((s) => s > zoom));
</script>

<section class="desk" aria-label="Preview">
  <div class="bar">
    {@render status()}
    {#if children}<span class="meta note">{@render children()}</span>{/if}
    <div class="zoom-group" role="group" aria-label="Zoom">
      <Button
        variant="ghost"
        size="sm"
        square
        icon={Minus}
        aria-label="Zoom out"
        disabled={zoomOut === undefined}
        onclick={() => zoomOut !== undefined && (zoom = zoomOut)}
      />
      <button type="button" class="zoom meta num" title="Actual size" onclick={() => (zoom = 100)}>{zoom}%</button>
      <Button
        variant="ghost"
        size="sm"
        square
        icon={Plus}
        aria-label="Zoom in"
        disabled={zoomIn === undefined}
        onclick={() => zoomIn !== undefined && (zoom = zoomIn)}
      />
    </div>
  </div>
  <div class="pages" style:--zoom={zoom / 100}>
    {#each pages as page, i (i)}
      <article class="page" aria-label="Page {i + 1} of {pages.length}">{@html page}</article>
    {/each}
  </div>
</section>

<style>
  .desk {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    background: var(--desk);
  }
  .bar {
    position: sticky;
    top: 0;
    left: 0;
    z-index: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4) var(--space-3) var(--space-6);
    background: var(--desk);
  }
  .note {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .zoom-group {
    display: flex;
    margin-inline-start: auto;
    flex: none;
    align-items: center;
    gap: var(--space-2);
  }
  .zoom {
    flex: none;
    min-width: 40px;
    height: var(--control-sm);
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    text-align: center;
    cursor: default;
  }
  .pages {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    padding: var(--space-3) var(--space-6) var(--space-10);
  }
  .page {
    flex: none;
    width: calc(min(612px, 100%) * var(--zoom));
    margin-inline: auto;
    border: 1px solid transparent;
    color: var(--page-ink);
    background: var(--page);
    box-shadow: var(--shadow-page);
  }
  .page :global(svg) {
    display: block;
    width: 100%;
    height: auto;
  }
  @media (hover: hover) {
    .zoom:hover {
      color: var(--ink);
      background: var(--hover);
    }
  }
</style>
