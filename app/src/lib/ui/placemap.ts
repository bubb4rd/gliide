// PlaceMap's cartography: a MapLibre style over OpenFreeMap's OpenMapTiles vector tiles, every color read from the
// tokens (design/tokens.css `--map-*`, ink, accent) at runtime, labels in the bundled Inter glyphs (static/glyphs/).

import type { Feature, FeatureCollection, Geometry } from "geojson";
import type {
  ExpressionSpecification,
  FilterSpecification,
  LayerSpecification,
  StyleSpecification,
} from "maplibre-gl";

/** One place on the map: what `places` gives, in the kit's own terms. */
export interface MapPlace {
  name: string;
  region: string;
  lat: number;
  lon: number;
  /** [west, south, east, north] */
  bbox: [number, number, number, number];
  /** GeoJSON geometry text (Polygon or MultiPolygon), or null for a point. */
  boundary: string | null;
}

export const TILES = "https://tiles.openfreemap.org/planet";
export const CREDIT = "OpenFreeMap © OpenMapTiles © OpenStreetMap";
export const CREDIT_SHORT = "© OpenStreetMap";
/** Our pin's label and the tiles' labels; Inter only covers Latin, so names prefer their Latin form. */
const REGULAR = ["inter-400"];
const MEDIUM = ["inter-500"];
const NAME: ExpressionSpecification = ["coalesce", ["get", "name:en"], ["get", "name:latin"], ["get", "name"]];

/** The tokens the map paints with, by their CSS name. */
const TOKENS = {
  land: "--map-land",
  park: "--map-park",
  water: "--map-water",
  road: "--map-road",
  casing: "--map-road-casing",
  ink: "--ink",
  ink2: "--ink-2",
  ink3: "--ink-3",
  ink4: "--ink-4",
  accent: "--accent",
  onAccent: "--on-accent",
} as const;
export type MapColors = Record<keyof typeof TOKENS, string>;

/**
 * The tokens as `rgba()` MapLibre can parse (it doesn't read `oklch`): each drawn to one pixel and read back, so the
 * browser does the conversion. Read from `el`, so a scoped theme counts.
 */
export function readColors(el: Element): MapColors {
  const style = getComputedStyle(el);
  const ctx = document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  const out = {} as MapColors;
  for (const [key, token] of Object.entries(TOKENS) as [keyof MapColors, string][]) {
    const css = style.getPropertyValue(token).trim();
    if (!ctx || !css) {
      out[key] = "rgba(0,0,0,0)";
      continue;
    }
    ctx.clearRect(0, 0, 1, 1);
    ctx.fillStyle = css;
    ctx.fillRect(0, 0, 1, 1);
    const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data;
    out[key] = `rgba(${r},${g},${b},${(a / 255).toFixed(3)})`;
  }
  return out;
}

/** A paint property and the token it takes. */
type Painted = { layer: string; prop: string; color: keyof MapColors };

const between = (z0: number, w0: number, z1: number, w1: number): ExpressionSpecification => [
  "interpolate",
  ["exponential", 1.5],
  ["zoom"],
  z0,
  w0,
  z1,
  w1,
];
const classIn = (...classes: string[]): ExpressionSpecification => ["in", ["get", "class"], ["literal", classes]];

/**
 * The layers, bottom to top, without their colors; `PAINTED` says which token each colored property takes, so the
 * first paint and a theme switch share one table.
 */
function layers(names: string[]): LayerSpecification[] {
  const notOurs: ExpressionSpecification = ["!", ["in", NAME, ["literal", names]]];
  const label = (size: number) => ({
    "text-field": NAME,
    "text-font": REGULAR,
    "text-size": size,
    "text-max-width": 8,
  });
  return [
    { id: "land", type: "background" },
    { id: "landcover", type: "fill", source: "tiles", "source-layer": "landcover", filter: classIn("wood", "grass") },
    { id: "park", type: "fill", source: "tiles", "source-layer": "park" },
    { id: "water", type: "fill", source: "tiles", "source-layer": "water" },
    {
      id: "waterway",
      type: "line",
      source: "tiles",
      "source-layer": "waterway",
      paint: { "line-width": between(8, 0.5, 14, 2) },
    },
    {
      id: "states",
      type: "line",
      source: "tiles",
      "source-layer": "boundary",
      filter: ["all", ["==", ["get", "admin_level"], 4], ["!=", ["get", "maritime"], 1]],
      paint: { "line-width": 0.75, "line-dasharray": [3, 2] },
    },
    {
      id: "road-minor",
      type: "line",
      source: "tiles",
      "source-layer": "transportation",
      minzoom: 12,
      filter: classIn("minor", "service"),
      layout: { "line-cap": "round", "line-join": "round" },
      paint: { "line-width": between(12, 0.5, 16, 6) },
    },
    {
      id: "road-casing",
      type: "line",
      source: "tiles",
      "source-layer": "transportation",
      filter: classIn("motorway", "trunk", "primary", "secondary", "tertiary"),
      layout: { "line-cap": "round", "line-join": "round" },
      paint: { "line-width": between(6, 1.5, 14, 9) },
    },
    {
      id: "road",
      type: "line",
      source: "tiles",
      "source-layer": "transportation",
      filter: classIn("motorway", "trunk", "primary", "secondary", "tertiary"),
      layout: { "line-cap": "round", "line-join": "round" },
      paint: { "line-width": between(6, 0.6, 14, 7) },
    },
    { id: "area", type: "fill", source: "area", paint: { "fill-opacity": 0.06 } },
    {
      id: "area-edge",
      type: "line",
      source: "area",
      layout: { "line-join": "round" },
      paint: { "line-width": 1.5, "line-dasharray": [2, 1.5] },
    },
    {
      id: "states-label",
      type: "symbol",
      source: "tiles",
      "source-layer": "place",
      maxzoom: 7,
      filter: ["all", ["==", ["get", "class"], "state"], notOurs],
      layout: label(11),
      paint: { "text-halo-width": 1.5 },
    },
    {
      id: "towns",
      type: "symbol",
      source: "tiles",
      "source-layer": "place",
      minzoom: 8,
      filter: ["all", classIn("town", "village", "suburb"), notOurs],
      layout: { ...label(11), "symbol-sort-key": ["get", "rank"] },
      paint: { "text-halo-width": 1.5 },
    },
    {
      id: "cities",
      type: "symbol",
      source: "tiles",
      "source-layer": "place",
      filter: ["all", ["==", ["get", "class"], "city"], notOurs],
      layout: { ...label(12), "symbol-sort-key": ["get", "rank"] },
      paint: { "text-halo-width": 1.5 },
    },
    // On top, so it's placed first and the tiles' labels make way for it.
    {
      id: "pin",
      type: "symbol",
      source: "pins",
      layout: {
        "icon-image": "pin",
        "icon-anchor": "bottom",
        "icon-allow-overlap": true,
        "text-field": ["get", "name"],
        "text-font": MEDIUM,
        "text-size": 13,
        "text-anchor": "left",
        // Beside the pin's head: 15px right of its axis, 21px above its point.
        "text-offset": [15 / 13, -21 / 13],
        "text-allow-overlap": true,
      },
      paint: { "text-halo-width": 2 },
    },
  ];
}

const PAINTED: Painted[] = [
  { layer: "land", prop: "background-color", color: "land" },
  { layer: "landcover", prop: "fill-color", color: "park" },
  { layer: "park", prop: "fill-color", color: "park" },
  { layer: "water", prop: "fill-color", color: "water" },
  { layer: "waterway", prop: "line-color", color: "water" },
  { layer: "states", prop: "line-color", color: "ink4" },
  { layer: "road-minor", prop: "line-color", color: "road" },
  { layer: "road-casing", prop: "line-color", color: "casing" },
  { layer: "road", prop: "line-color", color: "road" },
  { layer: "area", prop: "fill-color", color: "accent" },
  { layer: "area-edge", prop: "line-color", color: "accent" },
  { layer: "states-label", prop: "text-color", color: "ink3" },
  { layer: "states-label", prop: "text-halo-color", color: "land" },
  { layer: "towns", prop: "text-color", color: "ink3" },
  { layer: "towns", prop: "text-halo-color", color: "land" },
  { layer: "cities", prop: "text-color", color: "ink2" },
  { layer: "cities", prop: "text-halo-color", color: "land" },
  { layer: "pin", prop: "text-color", color: "ink" },
  { layer: "pin", prop: "text-halo-color", color: "land" },
];

/** Repaints a live map in `colors` (a theme switch). */
export function paint(map: { setPaintProperty(layer: string, prop: string, value: string): unknown }, colors: MapColors) {
  for (const { layer, prop, color } of PAINTED) map.setPaintProperty(layer, prop, colors[color]);
}

/** The pins, one per place, named. */
export function pins(places: MapPlace[]): FeatureCollection {
  return {
    type: "FeatureCollection",
    features: places.map((p) => ({
      type: "Feature",
      properties: { name: p.name },
      geometry: { type: "Point", coordinates: [p.lon, p.lat] },
    })),
  };
}

/** Each place's boundary, where it has one. */
export function areas(places: MapPlace[]): FeatureCollection {
  const features: Feature[] = [];
  for (const p of places) {
    if (!p.boundary) continue;
    try {
      features.push({ type: "Feature", properties: {}, geometry: JSON.parse(p.boundary) as Geometry });
    } catch {
      // A boundary that doesn't parse leaves the pin alone.
    }
  }
  return { type: "FeatureCollection", features };
}

/** Every place's box, together: [[west, south], [east, north]]. */
export function bounds(places: MapPlace[]): [[number, number], [number, number]] {
  const [w, s, e, n] = places.reduce(
    ([w, s, e, n], p) => [Math.min(w, p.bbox[0]), Math.min(s, p.bbox[1]), Math.max(e, p.bbox[2]), Math.max(n, p.bbox[3])],
    [Infinity, Infinity, -Infinity, -Infinity],
  );
  return [
    [w, s],
    [e, n],
  ];
}

export function mapStyle(places: MapPlace[], colors: MapColors): StyleSpecification {
  const style: StyleSpecification = {
    version: 8,
    glyphs: `${location.origin}/glyphs/{fontstack}/{range}.pbf`,
    sources: {
      tiles: { type: "vector", url: TILES },
      area: { type: "geojson", data: areas(places) },
      pins: { type: "geojson", data: pins(places) },
    },
    layers: layers(places.map((p) => p.name)),
  };
  for (const { layer, prop, color } of PAINTED) {
    const spec = style.layers.find((l) => l.id === layer);
    if (spec) spec.paint = { ...spec.paint, [prop]: colors[color] } as never;
  }
  return style;
}

/** The tiles' labels that would repeat a pin's name stay hidden; re-filtered when the places change. */
export function labelFilters(names: string[]): [string, FilterSpecification][] {
  const fresh = layers(names);
  return ["states-label", "towns", "cities"].map((id) => {
    const layer = fresh.find((l) => l.id === id) as { filter: FilterSpecification };
    return [id, layer.filter];
  });
}

/** Pin size in CSS pixels: the head (radius 10) and its point, with a 1.5px halo in land. */
const PIN_W = 28;
const PIN_H = 34;

/** The pin in accent, its eye in on-accent, haloed in land so it reads on any street; at device resolution. */
export function pinImage(colors: MapColors, ratio: number): { image: ImageData; ratio: number } {
  const canvas = document.createElement("canvas");
  canvas.width = Math.ceil(PIN_W * ratio);
  canvas.height = Math.ceil(PIN_H * ratio);
  const ctx = canvas.getContext("2d")!;
  ctx.scale(ratio, ratio);
  const cx = PIN_W / 2;
  const cy = 13;
  const r = 10;
  const tip = PIN_H - 2;
  // The point's sides are tangent to the head.
  const a = Math.acos(r / (tip - cy));
  ctx.beginPath();
  ctx.moveTo(cx, tip);
  ctx.arc(cx, cy, r, Math.PI / 2 + a, Math.PI / 2 - a + 2 * Math.PI);
  ctx.closePath();
  ctx.lineJoin = "round";
  ctx.lineWidth = 3;
  ctx.strokeStyle = colors.land;
  ctx.stroke();
  ctx.fillStyle = colors.accent;
  ctx.fill();
  ctx.beginPath();
  ctx.arc(cx, cy, 3.5, 0, 2 * Math.PI);
  ctx.fillStyle = colors.onAccent;
  ctx.fill();
  return { image: ctx.getImageData(0, 0, canvas.width, canvas.height), ratio };
}
