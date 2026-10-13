// Motion for elements Svelte removes (toasts). The duration comes from tokens.css; movement and blur run only without
// the reduced-motion preference, which keeps the opacity fade (DESIGN.md Motion budget).

import type { TransitionConfig } from "svelte/transition";

/** `ease-out`, the curve every enter and exit uses. */
const easeOut = (t: number) => 1 - (1 - t) ** 3;

/** Exit: opacity + 4px blur + a fixed -12px over `--dur-fast`. Reduced motion: opacity only. */
export function exit(_node: Element): TransitionConfig {
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  return {
    duration: parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--dur-fast")) || 0,
    easing: easeOut,
    css: (t, u) => (still ? `opacity: ${t}` : `opacity: ${t}; filter: blur(${u * 4}px); translate: 0 ${u * -12}px`),
  };
}
