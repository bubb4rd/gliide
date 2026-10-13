<!-- Text with each occurrence of a mark (an inserted keyword) wrapped in <mark class="mark">. -->
<script lang="ts">
  interface Props {
    text: string;
    marks: string[];
  }

  let { text, marks }: Props = $props();

  // Longest first, so "REST APIs" wins over "REST" where both are marks; case-insensitive and whole terms, like the
  // matcher ("Git" isn't marked inside "GitHub").
  const parts = $derived.by(() => {
    const terms = [...new Set(marks.filter((m) => m.trim()))].sort((a, b) => b.length - a.length);
    if (!terms.length) return [{ text, mark: false }];
    const alternatives = terms.map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|");
    const pattern = new RegExp(`(?<![\\p{L}\\p{N}])(${alternatives})(?![\\p{L}\\p{N}])`, "giu");
    // `split` with a capture group puts every match at an odd index.
    return text
      .split(pattern)
      .map((piece, i) => ({ text: piece, mark: i % 2 === 1 }))
      .filter((part) => part.text);
  });
</script>

{#each parts as part, i (i)}{#if part.mark}<mark class="mark">{part.text}</mark>{:else}{part.text}{/if}{/each}

<style>
  .mark {
    padding-inline: 1px;
    border-radius: 2px;
    color: inherit;
    background: var(--mark);
    box-decoration-break: clone;
    -webkit-box-decoration-break: clone;
  }
</style>
