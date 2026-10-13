<!--
  A panel anchored to an element, in the top layer (the native `popover` attribute). `mode="auto"` closes on Esc and
  outside clicks (menus, pickers); `mode="manual"` leaves open/close to the owner (hover and focus details).
  Opens with a 100ms opacity fade, closes instantly: menus are frequent, so no movement.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  interface Props extends Omit<HTMLAttributes<HTMLDivElement>, "popover"> {
    open: boolean;
    anchor: HTMLElement | undefined;
    placement?: "bottom-start" | "bottom-end" | "top-start" | "top-end";
    mode?: "auto" | "manual";
    /** Gap between the anchor and the panel, px. */
    offset?: number;
    /** Match the anchor's width at least (menus). */
    matchWidth?: boolean;
    element?: HTMLDivElement;
    children: Snippet;
  }

  let {
    open = $bindable(),
    anchor,
    placement = "bottom-start",
    mode = "auto",
    offset = 4,
    matchWidth = false,
    element = $bindable(),
    children,
    class: className,
    ...rest
  }: Props = $props();

  const EDGE = 8;

  function place() {
    if (!element || !anchor) return;
    const a = anchor.getBoundingClientRect();
    if (matchWidth) element.style.minWidth = `${a.width}px`;
    const p = element.getBoundingClientRect();
    const vw = document.documentElement.clientWidth;
    const vh = document.documentElement.clientHeight;

    const below = a.bottom + offset;
    const above = a.top - offset - p.height;
    let top = placement.startsWith("bottom") ? below : above;
    // Flip when the preferred side overflows and the other side has room.
    if (placement.startsWith("bottom") && below + p.height > vh - EDGE && above >= EDGE) top = above;
    if (placement.startsWith("top") && above < EDGE && below + p.height <= vh - EDGE) top = below;

    let left = placement.endsWith("start") ? a.left : a.right - p.width;
    left = Math.min(Math.max(left, EDGE), vw - EDGE - p.width);

    element.style.top = `${Math.max(top, EDGE)}px`;
    element.style.left = `${left}px`;
    element.style.maxHeight = `${vh - 2 * EDGE}px`;
  }

  // Opened by the owner or natively (a `popovertarget` invoker): either way, place it while open.
  $effect(() => {
    if (!element) return;
    const shown = element.matches(":popover-open");
    if (open) {
      if (!shown) element.showPopover();
      place();
    } else if (shown) {
      element.hidePopover();
    }
  });

  $effect(() => {
    if (!open) return;
    const reposition = () => place();
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    return () => {
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    };
  });
</script>

<div
  bind:this={element}
  popover={mode}
  class={["popover", className]}
  ontoggle={(e) => (open = e.newState === "open")}
  {...rest}
>
  {@render children()}
</div>

<style>
  .popover {
    position: fixed;
    inset: auto;
    margin: 0;
    padding: 0;
    min-width: 160px;
    max-width: min(420px, calc(100vw - 2 * var(--space-2)));
    overflow: auto;
    overscroll-behavior: contain;
    color: var(--ink);
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    background: var(--surface);
    box-shadow: var(--shadow-popover);
    opacity: 1;
    transition-property: opacity;
    transition-duration: var(--dur-instant);
    transition-timing-function: var(--ease-out);
  }
  @starting-style {
    .popover:popover-open {
      opacity: 0;
    }
  }
</style>
