<!--
  Meter: a share as a 48×4 pill, the part in `--accent` on a sunken track (a data series, not "good"). Any share above
  0 shows at least 2px. Its percent is in its accessible value; the number beside it is the caller's.
-->
<script lang="ts">
  interface Props {
    /** 0–1. */
    value: number;
    /** What it measures, for assistive tech ("Share of cost"). */
    label: string;
  }

  let { value, label }: Props = $props();

  const share = $derived(Math.min(1, Math.max(0, value)));
  const pct = $derived(Math.round(share * 100));
</script>

<span
  class="meter"
  role="meter"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={pct}
  aria-valuetext="{pct}%"
>
  {#if share > 0}<span class="fill" style:--share={share}></span>{/if}
</span>

<style>
  .meter {
    position: relative;
    display: inline-block;
    flex: none;
    width: var(--space-12);
    height: var(--space-1);
    border: 1px solid transparent;
    border-radius: var(--radius-full);
    background: var(--sunken);
    box-shadow: inset 0 0 0 1px var(--line);
    vertical-align: middle;
  }
  /* Over the track's edge: the pill's whole 4px, from its outer start. */
  .fill {
    position: absolute;
    inset-block: -1px;
    inset-inline-start: -1px;
    width: calc(var(--share) * (100% + 2px));
    min-width: 2px;
    border-radius: var(--radius-full);
    background: var(--accent);
  }

  @media (forced-colors: active) {
    .fill {
      forced-color-adjust: none;
      background: Highlight;
    }
  }
</style>
