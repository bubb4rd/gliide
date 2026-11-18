<!--
  Showcase: a picture as a panel of its own beside a screen's form, the way a sign-in page shows its product (the
  Startup screen). One of the `BACKDROPS`, as painted (no veil, no screen, no fade), covering a `--radius-xl` inset
  panel with the image outline. What goes over it (`children`) sits at its foot in a card on `--surface` at
  `--backdrop-card`, so the picture shows through it faintly; a card taller than the room left scrolls on its own. The
  picture is decorative: hidden from assistive tech, and gone in forced colors, where the card alone remains.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { BACKDROPS, type BackdropName } from "./Backdrop.svelte";

  interface Props {
    name: BackdropName;
    children?: Snippet;
  }

  let { name, children }: Props = $props();
</script>

<div class="showcase">
  <img src={BACKDROPS[name]} alt="" aria-hidden="true" decoding="async" draggable="false" />
  {#if children}<div class="card">{@render children()}</div>{/if}
</div>

<style>
  /* 32px in from the panel's edge: past 24px the card and the panel are independent surfaces, each its own radius. */
  .showcase {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    height: 100%;
    min-height: 0;
    padding: var(--space-8);
    overflow: hidden;
    border-radius: var(--radius-xl);
    background: var(--sunken);
    user-select: none;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: 50% 60%;
    pointer-events: none;
  }
  /* The image outline, over the picture and under the card. */
  .showcase::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow: inset 0 0 0 1px var(--image-outline);
    pointer-events: none;
  }
  .card {
    position: relative;
    z-index: 1;
    min-height: 0;
    max-height: 100%;
    overflow: auto;
    overscroll-behavior: contain;
    padding: var(--space-5);
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    background: color-mix(in oklab, var(--surface) var(--backdrop-card), transparent);
    box-shadow: var(--shadow-border);
    user-select: text;
  }
  @media (forced-colors: active) {
    img,
    .showcase::after {
      display: none;
    }
  }
</style>
