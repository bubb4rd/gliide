<!--
  A picture behind the top of a screen (Settings → Appearance; the Overview only). It fills the top 560px of its
  positioned parent and scrolls with it; the parent's content sits above it (positioned too). Paper veils it, so ink
  reads on it, and a 3px dot screen in paper lies over it; below its top 64px it fades out to paper over 496px
  (`static/backdrops/fade.png`: a smootherstep, its alpha dithered with blue noise at device pixels so it never bands):
  white in light, near black in dark. Decorative: hidden from assistive tech, and gone in forced colors.
-->
<script module lang="ts">
  /** The pictures a backdrop can show, bundled in `static/backdrops/` (where each came from in its README). */
  export const BACKDROPS = {
    meadow: "/backdrops/meadow.webp",
  } as const;
  export type BackdropName = keyof typeof BACKDROPS;
</script>

<script lang="ts">
  interface Props {
    name: BackdropName;
  }

  let { name }: Props = $props();
</script>

<div class="backdrop" aria-hidden="true">
  <img src={BACKDROPS[name]} alt="" decoding="async" draggable="false" />
</div>

<style>
  /* 560px: fade.png's height in CSS pixels (drawn at 2×, so each mask pixel is one Retina pixel); its 64px tile
     repeats across. */
  .backdrop {
    position: absolute;
    inset-block-start: 0;
    inset-inline: 0;
    height: 560px;
    overflow: hidden;
    pointer-events: none;
    user-select: none;
    mask: url("/backdrops/fade.png") 0 0 / 64px 560px repeat-x;
  }
  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    object-position: 50% 60%;
  }
  .backdrop::before,
  .backdrop::after {
    content: "";
    position: absolute;
    inset: 0;
    background: var(--paper);
  }
  .backdrop::before {
    opacity: var(--backdrop-veil);
  }
  .backdrop::after {
    opacity: var(--backdrop-screen);
    mask: url("/backdrops/screen.svg") 0 0 / 3px 3px;
  }
  @media (forced-colors: active) {
    .backdrop {
      display: none;
    }
  }
</style>
