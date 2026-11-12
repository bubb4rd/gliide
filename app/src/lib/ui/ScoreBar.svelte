<!--
  A fit score as one number with a bullet bar before it (DESIGN.md Applications): the review's score bar in small.
  The bar runs 0–100 and ends at the score that counts (on file once exported, else projected). Ink 3 is what the
  master had; green is what your edits added; amber is what the PDF lost (projected, but not read back), outlined; a
  tick marks the ceiling. The number keeps the column's edge. Every number is in its label, for assistive tech, and in
  its tooltip, each part beside the mark it stands for. Updates are instant: no motion, with one exception: with
  `rescoring` (the sidebar's Recent, while Extract or Suggest runs for the posting), the rise that call brings rolls
  up from the old score, odometer-style, each digit up to its new one over `--dur-roll`, in gain; landed, it's plain
  text again and holds there as long again, then fades back to ink. Once per call; reduced motion: no roll, the gain
  alone.

  `bar={false}` is the number alone, styled the same (500, ink; amber when the PDF lost points), with a percent sign
  and the same label and tooltip: where a fit score sits beside a name (the sidebar's Recent, the Overview's lists).
-->
<script lang="ts">
  import { untrack } from "svelte";
  import { ROLL_CELLS, roll, token } from "./motion";
  import { tip, type TipData, type TipRow } from "./tip.svelte";

  interface Props {
    /** After your edits; for a posting not sent yet, its fit score. 0–100. */
    projected: number;
    /** The master before your edits; none for a posting not tailored yet or an imported application. */
    now?: number | null;
    /** Read back from the exported PDF. */
    onFile?: number | null;
    /** The most the edits could reach. */
    ceiling?: number | null;
    /** The bar before the number (tables); `false`, the number alone, beside a name. */
    bar?: boolean;
    /** A model call that rescores the posting (Extract, Suggest) is running: the rise it brings rolls up. */
    rescoring?: boolean;
  }

  let { projected, now = null, onFile = null, ceiling = null, bar = true, rescoring = false }: Props = $props();

  const pct = (n: number) => Math.round(Math.min(100, Math.max(0, n)));
  const proj = $derived(pct(projected));
  const before = $derived(now === null ? null : pct(now));
  const file = $derived(onFile === null ? null : pct(onFile));
  const top = $derived(ceiling === null ? null : pct(ceiling));

  /** The score that counts: what an ATS reads once it's exported. */
  const score = $derived(file ?? proj);
  const gain = $derived(before !== null && score > before ? score - before : 0);
  const base = $derived(before === null ? score : Math.min(before, score));
  const lost = $derived(file !== null && file < proj ? proj - file : 0);

  /** The digits rolling up to `score`, each from its old one (null: the old number didn't have it) to its cell; null
   *  otherwise. A digit lower than its old one goes round: its cell in the strip's second 0–9. */
  let rolling = $state<{ from: number | null; to: number }[] | null>(null);
  const digits = (from: number, to: number) => {
    const was = String(from).padStart(String(to).length, " ");
    return [...String(to)].map((c, i) => {
      const old = was[i] === " " ? null : Number(was[i]);
      const d = Number(c);
      return { from: old, to: old === null || d >= old ? d : d + 10 };
    });
  };
  /** A rise just rolled: the number in gain, until it fades back to ink. */
  let rose = $state(false);
  /** A rescoring call is running, or ended under `--dur-roll` × 4 ago (the jobs refresh comes after it ends). */
  let armed = false;
  $effect(() => {
    if (rescoring) return void (armed = true);
    const timer = setTimeout(() => (armed = false), token("--dur-roll") * 4);
    return () => clearTimeout(timer);
  });

  let last = untrack(() => score);
  $effect(() => {
    const from = last;
    const to = (last = score);
    if (!armed || to <= from || untrack(() => lost)) return;
    armed = false;
    rose = true;
    if (!matchMedia("(prefers-reduced-motion: reduce)").matches) rolling = digits(from, to);
    // Landed: plain text again (no window, no mask) while the gain holds, then ink.
    const landed = setTimeout(() => (rolling = null), token("--dur-roll"));
    const settle = setTimeout(() => (rose = false), token("--dur-roll") * 2);
    return () => {
      clearTimeout(landed);
      clearTimeout(settle);
      rolling = null;
      rose = false;
    };
  });

  const label = $derived(
    [
      `Fit ${score}`,
      before !== null && (gain ? `${before} before your edits, +${gain} from them` : `${before} before your edits`),
      lost && `The PDF reads ${file}, ${lost} under the ${proj} projected`,
      top !== null && `Ceiling ${top}`,
    ]
      .filter(Boolean)
      .join(". "),
  );

  const parts = $derived.by((): TipData => {
    const rows: TipRow[] = [];
    if (before !== null) rows.push({ key: "base", label: "Before your edits", value: before });
    if (gain) rows.push({ key: "gain", label: "From your edits", value: `+${gain}`, tone: "gain" });
    if (lost) rows.push({ key: "lost", label: "Lost in the PDF", value: `−${lost}`, tone: "warn" });
    if (top !== null) rows.push({ key: "tick", label: "Ceiling", value: top });
    return {
      title: "Fit",
      value: score,
      tone: lost ? "warn" : undefined,
      rows,
      note: file !== null ? "Read back from the exported PDF" : undefined,
    };
  });
</script>

<span
  class={["score-bar", !bar && "score-bar--alone", lost && "score-bar--lost", rose && "score-bar--rose"]}
  role="img"
  aria-label={label}
  {@attach tip(() => parts)}
>
  {#if bar}
    <span class="track">
      {#if base > 0}<span class="seg seg--base" style:width="{base}%"></span>{/if}
      {#if gain}<span class="seg seg--gain" style:left="{before}%" style:width="{gain}%"></span>{/if}
      {#if lost}<span class="seg seg--lost" style:left="{file}%" style:width="{lost}%"></span>{/if}
      {#if top !== null}<span class="tick" style:left="{top}%"></span>{/if}
    </span>
    <span class="value num">{@render number()}</span>
  {:else}
    <span class="value num">{@render number()}%</span>
  {/if}
</span>

{#snippet number()}
  {#if rolling}<span class="roll"
      >{#each rolling as digit, i (i)}<span class="digit"
          ><span class="strip" {@attach roll(digit.from, digit.to)}
            >{#each ROLL_CELLS as cell, j (j)}<span>{cell}</span>{/each}</span
          ></span
        >{/each}</span
    >{:else}{score}{/if}
{/snippet}

<style>
  .score-bar {
    display: grid;
    grid-template-columns: minmax(var(--space-12), 1fr) 3ch;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .score-bar--alone {
    display: inline-block;
  }
  /* A sunken pill; segments are pills 2px apart; the ceiling crosses it. */
  .track {
    position: relative;
    display: block;
    height: 8px;
    border: 1px solid transparent;
    border-radius: var(--radius-full);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .seg {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    display: block;
    border-radius: var(--radius-full);
  }
  .seg--base {
    background: var(--ink-3);
  }
  .seg--gain,
  .seg--lost {
    margin-inline-start: 2px;
    max-width: calc(100% - 2px);
  }
  .seg--gain {
    background: var(--gain);
  }
  .seg--lost {
    background: var(--warn-soft);
    box-shadow: inset 0 0 0 1px var(--warn);
  }
  .tick {
    position: absolute;
    top: -4px;
    bottom: -4px;
    display: block;
    width: 2px;
    margin-inline-start: -1px;
    border-radius: var(--radius-full);
    background: var(--ink-2);
  }
  .value {
    text-align: end;
    font-weight: var(--weight-medium);
    color: var(--ink);
    transition: color var(--dur-base) var(--ease-out);
  }
  .score-bar--lost .value {
    color: var(--warn);
  }
  .score-bar--rose .value {
    color: var(--gain-text);
  }
  /* Rolling digits: each a column of 0–9 twice in a one-line window; `roll` moves it up to its cell. The window reaches
     4px past each side of the line (content-box: no room taken, the line itself never faded) and fades out there, so
     digits pass its edges softly instead of being cut. Laid on the line's top, so the digits sit on the same baseline
     as the percent sign. */
  .roll {
    display: inline-flex;
    vertical-align: top;
  }
  .digit {
    box-sizing: content-box;
    height: 1lh;
    padding-block: var(--space-1);
    margin-block: calc(-1 * var(--space-1));
    overflow: clip;
    mask-image: linear-gradient(
      transparent,
      currentColor var(--space-1),
      currentColor calc(100% - var(--space-1)),
      transparent
    );
  }
  .strip {
    display: flex;
    flex-direction: column;
  }

  @media (forced-colors: active) {
    .seg,
    .tick {
      forced-color-adjust: none;
      background: CanvasText;
    }
    .seg--gain {
      background: Highlight;
    }
    /* Shadows are dropped in forced colors: the lost part is an outline. */
    .seg--lost {
      background: Canvas;
      border: 1px solid CanvasText;
    }
  }
</style>
