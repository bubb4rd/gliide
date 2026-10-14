<!--
  The tooltip layer (DESIGN.md Tooltip), rendered once by the layout: shows what `tip()` opened, in the top layer,
  centered above its element (below when there's no room above), never past the window's edge. It never takes the
  pointer. Opens with a 100ms opacity fade; straight from another tooltip it moves at once; closes instantly.
-->
<script lang="ts">
  import Tooltip from "./Tooltip.svelte";
  import { closeTip, readTip, tipState } from "./tip.svelte";

  const id = $props.id();
  let element = $state<HTMLDivElement>();

  const EDGE = 8;
  const GAP = 6;

  const content = $derived(readTip(tipState.source));
  const anchor = $derived(content === null ? null : tipState.anchor);

  function place(target: HTMLElement) {
    if (!element) return;
    // Measure where it can take its full width.
    element.style.left = "0px";
    element.style.top = "0px";
    const a = target.getBoundingClientRect();
    const p = element.getBoundingClientRect();
    const vw = document.documentElement.clientWidth;
    const vh = document.documentElement.clientHeight;
    const above = a.top - GAP - p.height;
    const below = a.bottom + GAP;
    const top = above >= EDGE || below + p.height > vh - EDGE ? Math.max(above, EDGE) : below;
    const left = Math.min(Math.max(a.left + a.width / 2 - p.width / 2, EDGE), vw - EDGE - p.width);
    element.style.left = `${left}px`;
    element.style.top = `${top}px`;
  }

  // Show, move and resize with the content; content is read here so a change while open re-places it.
  $effect(() => {
    if (!element) return;
    const shown = element.matches(":popover-open");
    if (!anchor || content === null) {
      if (shown) element.hidePopover();
      return;
    }
    if (!shown) element.showPopover();
    place(anchor);
  });

  // While open: its element is described by it; scrolling, resizing, Esc or leaving the window closes it.
  $effect(() => {
    const target = anchor;
    if (!target) return;
    const own = !target.hasAttribute("aria-describedby");
    if (own) target.setAttribute("aria-describedby", id);
    const close = () => closeTip();
    const key = (e: KeyboardEvent) => e.key === "Escape" && closeTip();
    window.addEventListener("scroll", close, { capture: true, passive: true });
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    window.addEventListener("keydown", key);
    return () => {
      if (own) target.removeAttribute("aria-describedby");
      window.removeEventListener("scroll", close, { capture: true });
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("keydown", key);
    };
  });
</script>

<div bind:this={element} {id} popover="manual" role="tooltip" class={["layer", tipState.warm && "layer--warm"]}>
  {#if content !== null}<Tooltip {content} />{/if}
</div>

<style>
  .layer {
    position: fixed;
    inset: auto;
    margin: 0;
    padding: 0;
    overflow: visible;
    border: 0;
    color: var(--ink);
    background: transparent;
    pointer-events: none;
    opacity: 1;
    transition-property: opacity;
    transition-duration: var(--dur-instant);
    transition-timing-function: var(--ease-out);
  }
  .layer--warm {
    transition-duration: 0s;
  }
  @starting-style {
    .layer:popover-open {
      opacity: 0;
    }
  }
</style>
