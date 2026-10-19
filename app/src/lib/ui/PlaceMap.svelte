<!--
  The job's place on a map, as a maps search shows a town: a pin on it in accent with its name beside it, its boundary
  dashed, the towns and roads around it, framed to fit. Edge to edge (it fills the top of the job's preview pane, over
  the desk; whatever holds it draws its edges). Static (no pan or zoom); with `onopen` the whole map is a button that
  opens it in Maps. Tiles: OpenFreeMap's vector tiles drawn by MapLibre, every color from the tokens (repainted on a
  theme switch), labels in the bundled Inter. Until the tiles arrive, and when they can't, the place's name and region
  stand in, centered on land.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import type { Map as MapLibre } from "maplibre-gl";
  import {
    bounds,
    CREDIT,
    CREDIT_SHORT,
    labelFilters,
    mapStyle,
    paint,
    pinImage,
    pins,
    areas,
    readColors,
    type MapPlace,
  } from "./placemap";
  import { tip } from "./tip.svelte";

  interface Props {
    /** One or more; the first names the map. */
    places: MapPlace[];
    /** Makes the map a button ("Open in Maps"). */
    onopen?: () => void;
    /** The place alone, without the map, as when its tiles can't load. */
    unavailable?: boolean;
  }

  let { places, onopen, unavailable = false }: Props = $props();

  let host = $state<HTMLSpanElement>();
  let map = $state<MapLibre | null>(null);
  /** Its style is in, so layers can be repainted and sources set. */
  let styled = $state(false);
  let loaded = $state(false);
  let failed = $state(false);
  /** The places the map shows now. */
  let drawn: MapPlace[] | null = null;

  const first = $derived(places[0]);
  /** Every place's name, or the one place with its region: what the map is of. */
  const named = $derived(
    places.length > 1 ? places.map((p) => p.name).join(", ") : [first?.name, first?.region].filter(Boolean).join(", "),
  );
  const shown = $derived(loaded && !failed && !unavailable);
  /** Room above for a pin at the top (34px over its point), below for the credit, around for the labels. */
  const PADDING = { top: 40, bottom: 24, left: 24, right: 24 };

  onMount(() => {
    if (unavailable || !host) return;
    const el = host;
    let gone = false;
    let live: MapLibre | null = null;
    let theme: MutationObserver | null = null;
    void (async () => {
      const [lib, worker] = await Promise.all([
        import("maplibre-gl"),
        import("maplibre-gl/dist/maplibre-gl-worker.mjs?url"),
        import("maplibre-gl/dist/maplibre-gl.css"),
      ]).then(([lib, worker]) => [lib, worker] as const);
      if (gone) return;
      lib.setWorkerUrl(worker.default);
      const colors = readColors(el);
      const ratio = devicePixelRatio || 1;
      live = new lib.Map({
        container: el,
        style: mapStyle(places, colors),
        bounds: bounds(places),
        fitBoundsOptions: { padding: PADDING, maxZoom: 12 },
        interactive: false,
        attributionControl: false,
        fadeDuration: 0,
      });
      const m = live;
      drawn = places;
      const pin = pinImage(colors, ratio);
      m.addImage("pin", pin.image, { pixelRatio: pin.ratio });
      m.on("error", (e) => {
        // A glyph range Inter doesn't have isn't a failure; the tiles not arriving is.
        if ((e as { sourceId?: string }).sourceId === "tiles" && !loaded) failed = true;
      });
      m.once("load", () => (styled = true));
      m.once("idle", () => (loaded = true));
      m.on("resize", () => fit(m, false));
      const repaint = () => {
        const next = readColors(el);
        paint(m, next);
        m.updateImage("pin", pinImage(next, ratio).image);
      };
      theme = new MutationObserver(() => (styled ? repaint() : m.once("load", repaint)));
      theme.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
      map = m;
    })();
    return () => {
      gone = true;
      theme?.disconnect();
      live?.remove();
    };
  });

  function fit(m: MapLibre, animate: boolean) {
    if (!places.length) return;
    m.fitBounds(bounds(places), { padding: PADDING, maxZoom: 12, animate });
  }

  // New places on the same map (the location re-read): new pins, boundaries and framing.
  $effect(() => {
    const m = map;
    const now = places;
    if (!m || !styled || drawn === now) return;
    drawn = now;
    (m.getSource("pins") as { setData(d: unknown): void } | undefined)?.setData(pins(now));
    (m.getSource("area") as { setData(d: unknown): void } | undefined)?.setData(areas(now));
    for (const [layer, filter] of labelFilters(now.map((p) => p.name))) m.setFilter(layer, filter);
    fit(m, false);
  });
</script>

<div class="place enter">
  <svelte:element
    this={onopen ? "button" : "div"}
    class="surface"
    type={onopen ? "button" : undefined}
    role={onopen ? undefined : "img"}
    aria-label={onopen ? `Open ${named} in Maps` : `Map of ${named}`}
    onclick={onopen}
    {@attach tip(onopen ? "Open in Maps" : null)}
  >
    <span class="stand-in" class:hidden={shown} aria-hidden="true">
      <span class="name">{places.length > 1 ? named : first?.name}</span>
      {#if places.length === 1 && first?.region}<span class="meta">{first.region}</span>{/if}
      {#if failed || unavailable}<span class="meta">Map couldn’t load</span>{/if}
    </span>
    <span class="canvas" class:shown bind:this={host} aria-hidden="true"></span>
  </svelte:element>
  {#if !unavailable && !failed}
    <p class="credit" class:shown aria-hidden={!shown}>
      <span class="credit-full">{CREDIT}</span><span class="credit-short">{CREDIT_SHORT}</span>
    </p>
  {/if}
</div>

<style>
  .place {
    position: relative;
    height: 100%;
    min-height: 0;
    container-type: inline-size;
  }
  /* Edge to edge: it fills the top of its pane, and whatever holds it draws its edges. */
  .surface {
    position: relative;
    display: block;
    width: 100%;
    height: 100%;
    padding: 0;
    overflow: hidden;
    border: 0;
    font: inherit;
    color: inherit;
    text-align: inherit;
    background: var(--map-land);
    cursor: default;
  }
  /* Inside its edges, over the map (a ring outside would fall under the toolbar or the desk). */
  button.surface:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .canvas {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition-property: opacity;
    transition-duration: var(--dur-base);
    transition-timing-function: var(--ease-out);
  }
  .canvas.shown {
    opacity: 1;
  }
  /* MapLibre's own container class would make it relative; it stays laid over the stand-in. */
  .canvas:global(.maplibregl-map) {
    position: absolute;
  }
  .stand-in {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-0-5);
    padding: var(--space-3);
    text-align: center;
    transition-property: opacity;
    transition-duration: var(--dur-base);
    transition-timing-function: var(--ease-out);
  }
  .stand-in.hidden {
    opacity: 0;
  }
  .name {
    font-weight: var(--weight-medium);
    color: var(--ink);
    text-wrap: balance;
  }
  .credit {
    position: absolute;
    right: 0;
    bottom: 0;
    max-width: 100%;
    margin: 0;
    padding: 0 var(--space-1);
    overflow: hidden;
    border-start-start-radius: var(--radius-xs);
    font-size: var(--text-xs);
    line-height: var(--leading-xs);
    color: var(--ink-3);
    white-space: nowrap;
    background: color-mix(in oklab, var(--map-land) 80%, transparent);
    opacity: 0;
    transition-property: opacity;
    transition-duration: var(--dur-base);
    transition-timing-function: var(--ease-out);
    pointer-events: none;
  }
  .credit.shown {
    opacity: 1;
  }
  .credit-short {
    display: none;
  }
  /* Too narrow for every name: OpenStreetMap's, which the license requires, stays. */
  @container (width < 320px) {
    .credit-full {
      display: none;
    }
    .credit-short {
      display: inline;
    }
  }
</style>
