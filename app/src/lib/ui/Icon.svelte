<!--
  One icon library (Lucide). The rendered stroke matches the adjacent text weight (DESIGN.md Icons):
  regular 1.5px beside 400, medium 2px beside 500–600, bold 2.5px. `absoluteStrokeWidth` keeps those real pixels.
-->
<script lang="ts">
  import type { Component } from "svelte";
  import type { LucideProps } from "@lucide/svelte";

  interface Props {
    icon: Component<LucideProps>;
    size?: 12 | 14 | 16;
    weight?: "regular" | "medium" | "bold";
    class?: string;
    /** Names a standalone icon for assistive tech; omitted, the icon is decorative. */
    label?: string;
  }

  let { icon: Glyph, size = 16, weight = "regular", class: className, label }: Props = $props();

  // Mirrors --stroke-regular / --stroke-medium / --stroke-bold: Lucide takes the width as an attribute, not CSS.
  const STROKE = { regular: 1.5, medium: 2, bold: 2.5 } as const;
</script>

<Glyph
  {size}
  strokeWidth={STROKE[weight]}
  absoluteStrokeWidth
  class={["icon", className]}
  aria-hidden={label ? undefined : "true"}
  aria-label={label}
  role={label ? "img" : undefined}
/>

<style>
  :global(.icon) {
    flex: none;
  }
</style>
