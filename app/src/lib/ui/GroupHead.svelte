<!-- The head of a group of changes: the section label, then its count and points. -->
<script lang="ts">
  interface Props {
    label: string;
    meta?: string;
  }

  let { label, meta }: Props = $props();
</script>

<div class="group-head">
  <h3 class="label">{label}</h3>
  {#if meta}<span class="meta num">{meta}</span>{/if}
</div>

<style>
  .group-head {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-3) var(--space-1);
  }
  .meta {
    margin-inline-start: auto;
  }
</style>
