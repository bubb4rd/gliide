// Tooltips (DESIGN.md Tooltip): `{@attach tip(…)}` on any element; the layout's <Tooltips /> shows one at a time.
// Hover (a mouse resting on it) or keyboard focus opens it; leaving, a click, Esc, scrolling or resizing closes it.
// The first opens after a short rest, so crossing the screen never flashes them; while one is open, or just closed,
// the next opens at once, so reading along a chart or a toolbar feels instant.

import type { Attachment } from "svelte/attachments";

/**
 * A legend key: the mark a row stands for in the chart it explains (the score bar's segments and ceiling tick; the
 * keyword meter's cells: on the resume, added by an edit, missing).
 */
export type TipKey = "base" | "gain" | "lost" | "tick" | "have" | "add" | "missing";
/** A number's meaning, in the system's colors: points gained, or lost. */
export type TipTone = "gain" | "warn";

export interface TipRow {
  label: string;
  value?: string | number;
  key?: TipKey;
  tone?: TipTone;
}

/** A named list: its legend key, label and count over its items, which wrap (`tone` colors them). */
export interface TipGroup {
  label: string;
  key?: TipKey;
  tone?: TipTone;
  items: string[];
}

/**
 * A tooltip with data. Any part may be left out: a title with its value ("Fit 78"), the value alone, labeled rows
 * (with legend keys when they explain a chart), named lists, a note under them.
 */
export interface TipData {
  title?: string;
  /** The title's number, at the trailing edge; alone, the tooltip is just that number. */
  value?: string | number;
  tone?: TipTone;
  rows?: TipRow[];
  groups?: TipGroup[];
  /** A sentence under the rest, in ink 3. */
  note?: string;
}

/** A label (one line or a sentence), or data. */
export type TipContent = string | TipData;
/** What `tip()` takes: the content, or a function read when it opens (and again while open, as it changes). */
export type TipSource = TipContent | null | undefined | (() => TipContent | null | undefined);

/** Rest before the first tooltip opens. */
const OPEN_DELAY_MS = 300;
/** After one closes, the next opens at once for this long. */
const WARM_MS = 400;

class TipState {
  anchor = $state<HTMLElement | null>(null);
  source = $state.raw<TipSource>(null);
  /** Opened straight from another (or by focus): no fade. */
  warm = $state(false);
}

/** The open tooltip, for <Tooltips />. */
export const tipState = new TipState();

let timer: number | undefined;
let pending: HTMLElement | null = null;
let closedAt = -Infinity;

function open(anchor: HTMLElement, source: TipSource, warm: boolean) {
  clearTimeout(timer);
  pending = null;
  tipState.anchor = anchor;
  tipState.source = source;
  tipState.warm = warm;
}

function request(anchor: HTMLElement, source: TipSource) {
  if (tipState.anchor === anchor) return;
  if (tipState.anchor || performance.now() - closedAt < WARM_MS) return open(anchor, source, true);
  clearTimeout(timer);
  pending = anchor;
  timer = window.setTimeout(() => open(anchor, source, false), OPEN_DELAY_MS);
}

/** Closes the tooltip, or only `anchor`'s (open or about to open). */
export function closeTip(anchor?: HTMLElement) {
  if (!anchor || pending === anchor) {
    clearTimeout(timer);
    pending = null;
  }
  if (!tipState.anchor || (anchor && tipState.anchor !== anchor)) return;
  closedAt = performance.now();
  tipState.anchor = null;
  tipState.source = null;
}

/** What a source shows right now; nothing for null, undefined or an empty string. */
export function readTip(source: TipSource): TipContent | null {
  const content = typeof source === "function" ? source() : source;
  return content == null || content === "" ? null : content;
}

/**
 * Gives an element a tooltip: `<span {@attach tip("Fit score")}>`. Pass a function for content that changes or costs
 * something to build (`tip(() => describe(day))`): it's read only while the tooltip is open. Nothing (null, undefined,
 * "") shows no tooltip. The element keeps its own accessible name; the tooltip is the sighted reading of it, linked by
 * `aria-describedby` while open. On a component, the component must spread its rest props onto its element.
 */
export function tip(source: TipSource): Attachment<HTMLElement> {
  return (node) => {
    if (source == null || source === "") return;
    // A click dismisses it until the pointer leaves, as native tooltips do.
    let clicked = false;

    const enter = (e: PointerEvent) => {
      if (e.pointerType === "mouse" && !clicked && readTip(source) !== null) request(node, source);
    };
    const leave = () => {
      clicked = false;
      closeTip(node);
    };
    const press = () => {
      clicked = true;
      closeTip(node);
    };
    const focus = () => {
      if (node.matches(":focus-visible") && readTip(source) !== null) open(node, source, true);
    };
    const blur = () => closeTip(node);
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || tipState.anchor !== node) return;
      closeTip(node);
      e.stopPropagation();
    };

    node.addEventListener("pointerenter", enter);
    node.addEventListener("pointerleave", leave);
    node.addEventListener("pointerdown", press);
    node.addEventListener("focus", focus);
    node.addEventListener("blur", blur);
    node.addEventListener("keydown", key);
    return () => {
      node.removeEventListener("pointerenter", enter);
      node.removeEventListener("pointerleave", leave);
      node.removeEventListener("pointerdown", press);
      node.removeEventListener("focus", focus);
      node.removeEventListener("blur", blur);
      node.removeEventListener("keydown", key);
      closeTip(node);
    };
  };
}
