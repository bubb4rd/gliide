<!--
  A model call writing a pane (the review's first load, the cover letter): a ring round the pane's inside edge, over
  whatever it holds, in its positioned parent. A steady thin `--accent` outline, a faint inner glow, and a band of
  `--accent-text` (the sweep's profile: clear, accent, clear) travelling round it every `--dur-ring`, a blurred copy of
  the band lighting the paper just inside. Square: the pane is bounded by hairlines, not a card. Fades in and out over
  `--dur-fast`. Reduced motion: no band; the outline in accent, breathing as `.sweep`'s fallback. Forced colors: a
  2px `Highlight` border, breathing. Decorative: the stage's words (`Thinking`) say what's happening.
-->
<script lang="ts">
  import { fadeOut } from "./motion";

  let { on }: { on: boolean } = $props();
</script>

{#if on}
  <div class="ring" aria-hidden="true" out:fadeOut>
    <span class="halo"></span>
    <span class="line"></span>
    <span class="band"></span>
  </div>
{/if}

<style>
  @property --ring-angle {
    syntax: "<angle>";
    inherits: false;
    initial-value: 0deg;
  }

  .ring {
    position: absolute;
    inset: 0;
    z-index: 2;
    pointer-events: none;
    /* The halo lights only the pane's own paper. */
    overflow: hidden;
    animation: fade var(--dur-fast) var(--ease-out) both;
  }
  .ring > span {
    position: absolute;
    inset: 0;
  }
  /* Reduced motion and forced colors: a still outline, breathing. */
  .line {
    box-shadow: inset 0 0 0 1px var(--accent);
    animation: fade-breathe var(--dur-loop) var(--ease-out) infinite alternate;
  }
  .band,
  .halo {
    display: none;
  }

  @media (prefers-reduced-motion: no-preference) and (forced-colors: none) {
    .line {
      box-shadow: inset 0 0 0 1px var(--ring-line);
      animation: none;
    }
    .band,
    .halo {
      display: block;
      border: 0 solid transparent;
      /* A fifth of a turn: bright at its middle, clear at both ends. */
      border-image: conic-gradient(
          from var(--ring-angle),
          transparent 0deg,
          var(--accent-text) 36deg,
          transparent 72deg,
          transparent 360deg
        )
        1;
      animation: ring-travel var(--dur-ring) linear infinite;
    }
    .band {
      border-width: 2px;
    }
    .halo {
      border-width: var(--space-2);
      filter: blur(var(--space-2));
      opacity: var(--ring-halo);
    }
  }
  @keyframes ring-travel {
    to {
      --ring-angle: 360deg;
    }
  }

  @media (forced-colors: active) {
    .ring {
      box-shadow: none;
    }
    .line {
      box-shadow: none;
      border: 2px solid Highlight;
    }
  }
</style>
