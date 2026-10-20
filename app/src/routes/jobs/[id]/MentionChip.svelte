<!--
  A part an ask is limited to, as a chip in the accent where it sits in the ask's words: `@` and the entry's name, or,
  for a quoted line (`quote`), a quote mark and the line in quotes; either truncates within the room it has, the full
  label in its tooltip. In the composer it's one piece of the text (Backspace takes it whole); in a sent ask, it reads
  in its place.
-->
<script lang="ts">
  import AtSign from "@lucide/svelte/icons/at-sign";
  import Quote from "@lucide/svelte/icons/quote";
  import { Icon, tip } from "$lib/ui";
  import { mentionName } from "./thread";

  interface Props {
    label: string;
    quote?: boolean;
  }

  let { label, quote = false }: Props = $props();

  const name = $derived(mentionName({ label, quote }));
</script>

<span class={["chip", quote && "chip--quote"]} {@attach tip(label)}>
  <Icon icon={quote ? Quote : AtSign} size={12} weight="medium" />
  {#if quote}
    <span class="chip__label"><span aria-hidden="true">“</span><span class="chip__cut">{name}</span><span
        aria-hidden="true">”</span
      ></span>
  {:else}
    <span class="chip__label chip__cut">{name}</span>
  {/if}
</span>

<style>
  /* As an accent Badge, but it may shrink: an inline box that truncates instead of widening its line. */
  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-0-5);
    min-width: 0;
    max-width: 100%;
    height: 20px;
    padding-inline: var(--space-1-5) var(--space-2);
    border-radius: var(--radius-full);
    border: 1px solid transparent;
    font: var(--weight-medium) var(--text-xs) / 1 var(--font-ui);
    letter-spacing: 0.01em;
    vertical-align: 1px;
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  /* A quoted line is cut short enough to sit among the words around it; its tooltip has it whole. */
  .chip--quote {
    max-width: min(100%, 24em);
  }
  .chip :global(.icon) {
    flex: none;
  }
  /* A quote keeps its closing mark: the line inside it is what gets cut. */
  .chip__label {
    display: flex;
    min-width: 0;
  }
  .chip__cut {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
