<!--
  The desk: the resume pages (live SVG) on a darker surface, the only object with real depth. The bar holds the
  page-status badge, optional meta, and zoom. 100% fits the page to the desk's width (at most US Letter at 96 dpi,
  816px), with a narrow margin so the page reads large in its pane. With `above` (the job's map), the pane splits: that
  fills its top, edge to edge over a hairline, and the desk moves down under it. It takes the height the page leaves
  (at least 160px): at 100% the page shrinks so the two fill the pane exactly, the page's foot a margin from the
  pane's, never scrolling.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import Minus from "@lucide/svelte/icons/minus";
  import Plus from "@lucide/svelte/icons/plus";
  import Button from "./Button.svelte";
  import { tip } from "./tip.svelte";

  interface Props {
    /** One SVG document per page, from our renderer. */
    pages: string[];
    /** The page-status badge. */
    status: Snippet;
    /** Percent, 50–200. */
    zoom?: number;
    /** Between the badge and zoom ("Highlights show what changed"). */
    children?: Snippet;
    /** Over the desk, across the pane, sized by the page under it (the job's map). */
    above?: Snippet;
  }

  let { pages, status, zoom = $bindable(100), children, above }: Props = $props();

  const STEPS = [50, 67, 75, 90, 100, 110, 125, 150, 175, 200];
  const zoomOut = $derived(STEPS.findLast((s) => s < zoom));
  const zoomIn = $derived(STEPS.find((s) => s > zoom));

  // ── Fitting the page under `above` ──
  /** The page's widest (US Letter at 96 dpi), its inset from the desk's sides and foot, and above's least height. */
  const PAGE_MAX = 816;
  const INSET = 12;
  const TOP = 8;
  const ABOVE_MIN = 160;
  /** The page never shrinks below this share of the desk's width; past it, the desk scrolls. */
  const PAGE_LEAST = 0.6;
  let paneWidth = $state(0);
  let paneHeight = $state(0);
  let barHeight = $state(0);
  /** Height over width, from the first page's viewBox (US Letter until one renders). */
  const aspect = $derived.by(() => {
    const box = pages[0]?.match(/viewBox="\s*[-\d.]+[\s,]+[-\d.]+[\s,]+([\d.]+)[\s,]+([\d.]+)/);
    const ratio = box ? Number(box[2]) / Number(box[1]) : NaN;
    return Number.isFinite(ratio) && ratio > 0 ? ratio : 11 / 8.5;
  });
  /** The page's height at `width`: its sheet scaled inside a 1px (transparent) border. */
  const sheet = (width: number) => (width - 2) * aspect + 2;
  /** The page's width at 100% and the height left above the desk, in px; null without `above`. */
  const fit = $derived.by(() => {
    if (!above || !paneWidth || !paneHeight) return null;
    const across = Math.min(PAGE_MAX, paneWidth - 2 * INSET);
    /** What the desk needs besides its page. */
    const room = paneHeight - barHeight - TOP - INSET;
    const width = Math.floor(Math.max(across * PAGE_LEAST, Math.min(across, (room - ABOVE_MIN - 2) / aspect + 2)));
    const height = Math.floor(Math.max(ABOVE_MIN, room - sheet(width)));
    return { width, height };
  });
</script>

<div class="pane" bind:clientWidth={paneWidth} bind:clientHeight={paneHeight}>
  {#if above && fit}
    <div class="above" style:height="{fit.height}px">{@render above()}</div>
  {/if}
  <section class="desk" aria-label="Preview">
    <div class="bar" bind:offsetHeight={barHeight}>
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
        <button type="button" class="zoom meta num" {@attach tip("Actual size")} onclick={() => (zoom = 100)}
          >{zoom}%</button
        >
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
    <div
      class={["pages", fit && "pages--fit"]}
      style:--zoom={zoom / 100}
      style:--fit={fit ? `${fit.width}px` : null}
    >
      {#each pages as page, i (i)}
        <article class="page" aria-label="Page {i + 1} of {pages.length}">{@html page}</article>
      {/each}
    </div>
  </section>
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .desk {
    flex: 1 1 0;
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
    padding: var(--space-3);
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
    padding: var(--space-2) var(--space-3) var(--space-10);
  }
  .pages--fit {
    padding-block-end: var(--space-3);
  }
  .above {
    flex: none;
    border-block-end: 1px solid var(--line);
  }
  .page {
    flex: none;
    width: calc(var(--fit, min(816px, 100%)) * var(--zoom));
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
