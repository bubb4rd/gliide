<!--
  StepKey: a legend's swatch for a data step (an 8×8 square, radius 2), so a legend beside a `StackedBar` or a chart's
  tooltip matches its marks. Decorative: the label beside it names the series.
-->
<script lang="ts">
  interface Props {
    /** 1 (lightest) to 4 (the accent), as `Heatmap` and `StackedBar` (`segmentStep`) shade them. */
    step: 1 | 2 | 3 | 4;
  }

  let { step }: Props = $props();
</script>

<span class="step-key" style:background="var(--accent-step-{step})" aria-hidden="true"></span>

<style>
  .step-key {
    display: inline-block;
    flex: none;
    width: var(--space-2);
    height: var(--space-2);
    border-radius: calc(var(--radius-xs) / 2);
    vertical-align: middle;
  }

  @media (forced-colors: active) {
    .step-key {
      forced-color-adjust: none;
    }
  }
</style>
