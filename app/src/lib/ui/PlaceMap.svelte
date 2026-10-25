<!--
  The job's place on a map, as a maps search shows a town: a pin on it in accent with its name beside it, its boundary
  dashed, the towns and roads around it, framed to fit, and a label on top (a pin, filled in accent as the map's, then
  the place and its region).
  Edge to edge (it fills the top of the job's preview pane, over the desk; whatever holds it draws its edges). Static
  (no pan or zoom); with `onopen` the whole map is a button that opens it in Maps. Several places: all of them framed
  at first, the label saying "3 locations" or the place you're on, and a pager at the bottom (‹ All › … ‹ 2 / 3 ›)
  that steps the camera from place to place and back to all; its count shows them all again. Tiles: OpenFreeMap's
  vector tiles drawn by MapLibre, every color from the tokens (repainted on a theme switch), labels in the bundled
  Inter. Until the tiles arrive, and when they can't, the place's name and region stand in, centered on land.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import type { Map as MapLibre } from "maplibre-gl";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import MapPin from "@lucide/svelte/icons/map-pin";
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
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { tip } from "./tip.svelte";

  interface Props {
    /** One or more; the first names the map. */
    places: MapPlace[];
    /** The place the map is framed on, by index; null: all of them. Reset to null when `places` changes. */
    current?: number | null;
    /** Makes the map a button ("Open in Maps"). */
    onopen?: () => void;
    /** The place alone, without the map, as when its tiles can't load. */
    unavailable?: boolean;
  }

  let { places, current = $bindable(null), onopen, unavailable = false }: Props = $props();

  let host = $state<HTMLSpanElement>();
  let map = $state<MapLibre | null>(null);
  /** Its style is in, so layers can be repainted and sources set. */
  let styled = $state(false);
  let loaded = $state(false);
  let failed = $state(false);
  /** The places the map shows now. */
  let drawn: MapPlace[] | null = null;
  /** The place the camera is framed on now (null: all). */
  let framed: number | null = null;

  const first = $derived(places[0]);
  const many = $derived(places.length > 1);
  /** `current`, when it names one of `places`. */
  const at = $derived(current !== null && current >= 0 && current < places.length ? current : null);
  const here = $derived(at === null ? null : places[at]);
  /** What the camera frames: the place you're on, or all of them. */
  const view = $derived(here ? [here] : places);
  /** The one place the map is of now: the place you're on, or the only one; null while it shows several. */
  const one = $derived(here ?? (many ? null : first));
  /** Every place's name, or the one place with its region: what the map is of. */
  const named = $derived(
    one ? [one.name, one.region].filter(Boolean).join(", ") : places.map((p) => p.name).join(", "),
  );
  const shown = $derived(loaded && !failed && !unavailable);
  /**
   * Room above for the label over the top (28px, 8px in) and a pin under it (34px over its point), below for the
   * credit (with several places, the pager), around for the labels.
   */
  const PADDING = $derived({ top: 72, bottom: many ? 48 : 24, left: 24, right: 24 });

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
        bounds: bounds(view),
        fitBoundsOptions: { padding: PADDING, maxZoom: 12 },
        interactive: false,
        attributionControl: false,
        fadeDuration: 0,
      });
      const m = live;
      drawn = places;
      framed = at;
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
    if (!view.length) return;
    const still = !host || matchMedia("(prefers-reduced-motion: reduce)").matches;
    const duration = host ? parseFloat(getComputedStyle(host).getPropertyValue("--dur-base")) : 0;
    m.fitBounds(bounds(view), { padding: PADDING, maxZoom: 12, animate: animate && !still, duration });
  }

  /** One place on (`step` 1) or back (-1), through all of them between the last and the first. */
  function go(step: 1 | -1) {
    const n = places.length;
    const next = (at ?? (step > 0 ? -1 : n)) + step;
    current = next < 0 || next >= n ? null : next;
  }

  // New places on the same map (the location re-read): new pins, boundaries and framing, on all of them.
  // Another place chosen: the camera moves to it.
  $effect(() => {
    const m = map;
    const now = places;
    const on = at;
    if (!m || !styled) return;
    if (drawn !== now) {
      drawn = now;
      framed = null;
      if (current !== null) current = null;
      (m.getSource("pins") as { setData(d: unknown): void } | undefined)?.setData(pins(now));
      (m.getSource("area") as { setData(d: unknown): void } | undefined)?.setData(areas(now));
      for (const [layer, filter] of labelFilters(now.map((p) => p.name))) m.setFilter(layer, filter);
      fit(m, false);
      return;
    }
    if (on === framed) return;
    framed = on;
    fit(m, true);
  });
</script>

<div class={["place", "enter", many && "many"]}>
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
      <span class="name">{many ? places.map((p) => p.name).join(", ") : first?.name}</span>
      {#if !many && first?.region}<span class="meta">{first.region}</span>{/if}
      {#if failed || unavailable}<span class="meta">Map couldn’t load</span>{/if}
    </span>
    <span class="canvas" class:shown bind:this={host} aria-hidden="true"></span>
  </svelte:element>
  {#if shown}
    <p class="where float" aria-live="polite">
      <Icon icon={MapPin} size={14} weight="medium" />
      {#if one}
        <span class="where-name">{one.name}</span>
        {#if one.region}<span class="where-region">{one.region}</span>{/if}
      {:else}
        <span class="where-name">{places.length} locations</span>
      {/if}
    </p>
  {/if}
  {#if many && shown}
    <div class="pager float" role="group" aria-label="Locations">
      <Button
        variant="ghost"
        size="sm"
        square
        icon={ChevronLeft}
        aria-label="Previous location"
        {@attach tip("Previous location")}
        onclick={() => go(-1)}
      />
      <button
        type="button"
        class="count meta num"
        aria-label={here ? `Location ${(at ?? 0) + 1} of ${places.length}; show all` : `All ${places.length} locations`}
        {@attach tip(here ? "Show all" : null)}
        onclick={() => (current = null)}>{here ? `${(at ?? 0) + 1} / ${places.length}` : "All"}</button
      >
      <Button
        variant="ghost"
        size="sm"
        square
        icon={ChevronRight}
        aria-label="Next location"
        {@attach tip("Next location")}
        onclick={() => go(1)}
      />
    </div>
  {/if}
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
  /* Beside the pager, sooner. */
  @container (width < 400px) {
    .many .credit-full {
      display: none;
    }
    .many .credit-short {
      display: inline;
    }
  }

  /* Several places: the label and the pager, surfaces over the map 8px in from its edges, fading in with it. */
  .float {
    position: absolute;
    left: var(--space-2);
    display: flex;
    align-items: center;
    max-width: calc(100% - 2 * var(--space-2));
    height: var(--control-md);
    margin: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: var(--surface);
    box-shadow: var(--shadow-border);
    transition-property: opacity;
    transition-duration: var(--dur-base);
    transition-timing-function: var(--ease-out);
  }
  @starting-style {
    .float {
      opacity: 0;
    }
  }
  .where {
    top: var(--space-2);
    gap: var(--space-1-5);
    padding-inline: calc(var(--space-2) - 2px) var(--space-2);
    white-space: nowrap;
  }
  /* Filled, as the map's own pin: accent, its eye in --on-accent. */
  .where :global(svg) {
    flex: none;
    color: var(--accent);
  }
  .where :global(svg path) {
    fill: currentColor;
  }
  .where :global(svg circle) {
    fill: var(--on-accent);
    stroke: none;
  }
  .where-name,
  .where-region {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .where-name {
    flex: 0 1 auto;
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  /* The region gives way first. */
  .where-region {
    flex: 0 100 auto;
    font-size: var(--text-sm);
    color: var(--ink-3);
  }
  /* Radius 8 = 6 inner + 1 padding + 1 border. */
  .pager {
    bottom: var(--space-2);
    padding: 1px;
  }
  .count {
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
  @media (hover: hover) {
    .count:hover {
      color: var(--ink);
      background: var(--hover);
    }
  }
</style>
