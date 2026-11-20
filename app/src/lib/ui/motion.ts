// Motion for elements Svelte removes (toasts, the setup's steps, the working ring), and the sidebar fit's rolling
// digits. Durations and distances come from tokens.css; movement and blur run only without the reduced-motion
// preference, which keeps the opacity fade (DESIGN.md Motion budget).

import type { Attachment } from "svelte/attachments";
import type { TransitionConfig } from "svelte/transition";

/** `ease-out`, the curve every enter and exit uses. */
const easeOut = (t: number) => 1 - (1 - t) ** 3;

/** A duration or distance token from tokens.css, in ms or px. */
export const token = (name: string) =>
  parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name)) || 0;

/**
 * Exit: opacity + 4px blur + a fixed 12px over `--dur-fast`: up (-12px), or sideways with `side` (-1 toward the
 * leading edge, 1 toward the trailing one), as a step leaves for the next or the one before. Reduced motion: opacity
 * only.
 */
export function exit(_node: Element, { side = 0 }: { side?: -1 | 0 | 1 } = {}): TransitionConfig {
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const blur = token("--enter-blur");
  const away = token("--exit-y");
  return {
    duration: token("--dur-fast"),
    easing: easeOut,
    css: (t, u) => {
      if (still) return `opacity: ${t}`;
      const move = side ? `${side * -away * u}px 0` : `0 ${away * u}px`;
      return `opacity: ${t}; filter: blur(${u * blur}px); translate: ${move}`;
    },
  };
}

/** A status going away (the working ring): opacity only over `--dur-fast`, in every motion preference. */
export function fadeOut(_node: Element): TransitionConfig {
  return { duration: token("--dur-fast"), easing: easeOut, css: (t) => `opacity: ${t}` };
}

/** A rolling digit's strip: 0–9 twice, one cell a line, so a digit always rolls up to the next one round. */
export const ROLL_CELLS = "01234567890123456789";

export interface RollDigit {
  from: number | null;
  to: number;
}

/**
 * Each digit of `to` paired with its previous digit in `from` (null when `from` had fewer digits), wrapping past 9
 * into the strip's second 0–9 when the new digit is lower so every column rolls upward.
 */
export function rollDigits(from: number, to: number): RollDigit[] {
  const was = String(from).padStart(String(to).length, " ");
  return [...String(to)].map((c, i) => {
    const old = was[i] === " " ? null : Number(was[i]);
    const d = Number(c);
    return { from: old, to: old === null || d >= old ? d : d + 10 };
  });
}

/**
 * A digit rolling up, odometer-style (a rise in the sidebar's fit or a review's landing score): on its strip
 * (`ROLL_CELLS`, clipped to one line), from cell `from` up to cell `to` over `--dur-roll`, `--ease-emphasized`, so it
 * spins fast and settles. `from` null: a digit the old number didn't have, rolling up from 0 as it fades in. Callers
 * skip it under reduced motion.
 */
export function roll(from: number | null, to: number): Attachment<HTMLElement> {
  return (node) => {
    const cell = 100 / ROLL_CELLS.length;
    const at = (n: number) => `0 ${-n * cell}%`;
    node.style.translate = at(to);
    const easing = getComputedStyle(document.documentElement).getPropertyValue("--ease-emphasized").trim();
    const spin = node.animate(
      [
        { translate: at(from ?? 0), opacity: from === null ? 0 : 1 },
        { translate: at(to), opacity: 1 },
      ],
      { duration: token("--dur-roll"), easing },
    );
    return () => spin.cancel();
  };
}

/**
 * One-shot fill for a score bar segment arriving from a loading state (`0%` → `widthPct%` over `--dur-roll`,
 * `--ease-emphasized`). Under reduced motion: fades in over `--dur-base` without moving.
 */
export function fillBar(node: HTMLElement, widthPct: number): Animation {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
    return node.animate([{ opacity: 0 }, { opacity: 1 }], {
      duration: token("--dur-base"),
      easing: "ease-out",
    });
  }
  const easing = getComputedStyle(document.documentElement).getPropertyValue("--ease-emphasized").trim();
  return node.animate([{ width: "0%" }, { width: `${widthPct}%` }], {
    duration: token("--dur-roll"),
    easing,
  });
}
