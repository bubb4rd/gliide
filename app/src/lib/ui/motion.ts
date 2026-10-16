// Motion for elements Svelte removes (toasts, the setup's steps). Durations and distances come from tokens.css;
// movement and blur run only without the reduced-motion preference, which keeps the opacity fade (DESIGN.md Motion
// budget).

import type { TransitionConfig } from "svelte/transition";

/** `ease-out`, the curve every enter and exit uses. */
const easeOut = (t: number) => 1 - (1 - t) ** 3;

const token = (name: string) => parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name)) || 0;

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
