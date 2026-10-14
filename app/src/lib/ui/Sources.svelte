<!--
  Where a list's postings come from: a lead in meta ("Searches"), each source's icon as a 16px `CompanyMark` (shown
  as the brand publishes it, scaled into the square and rounded like the tile: never recolored, filtered or outlined)
  with its name in a tooltip and to assistive tech, then an optional note in meta ("and 260+ more boards through
  freehire"). One line; it wraps between the icons and the note when it must. Feed → Search's strip.
-->
<script lang="ts" module>
  export interface Source {
    name: string;
    /** The brand's icon, bundled (static/sources/); none: the blank tile. */
    src: string | null;
  }
</script>

<script lang="ts">
  import CompanyMark from "./CompanyMark.svelte";
  import { tip } from "./tip.svelte";

  interface Props {
    sources: Source[];
    /** The lead, which also names the list ("Searches"). */
    label: string;
    /** After the icons, in meta. */
    note?: string;
  }

  let { sources, label, note }: Props = $props();
</script>

<div class="sources">
  <span class="meta">{label}</span>
  <ul class="marks" aria-label={label}>
    {#each sources as source (source.name)}
      <li {@attach tip(source.name)}>
        <CompanyMark size={16} src={source.src} />
        <span class="visually-hidden">{source.name}</span>
      </li>
    {/each}
  </ul>
  {#if note}<span class="meta">{note}</span>{/if}
</div>

<style>
  .sources {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-1) var(--space-2);
    min-width: 0;
  }
  .marks {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  /* The tooltip's anchor: the mark itself, not a line box around it. */
  .marks > li {
    display: block;
  }
</style>
