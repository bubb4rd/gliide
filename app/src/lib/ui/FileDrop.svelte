<!--
  FileDrop: a dashed well to drop a file on. The window takes the drop (Tauri hands its paths to `onDragDropEvent`), so
  the screen tracks the drag and sets `over` while files are over the well; give it a `data-*` attribute to find it by.
  `sm`: inside a popover (the job header's photo panel, `LogoDrop`), its corners concentric with the popover's; `md`:
  on its own, as a card is, with the formats it takes under its line and a way to choose a file (`children`) under
  them. `busy`: the dropped file is being taken. Its words are the caller's, so they can say what dropping does now.
-->
<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import type { LucideProps } from "@lucide/svelte";
  import Icon from "./Icon.svelte";

  interface Props extends HTMLAttributes<HTMLDivElement> {
    icon: Component<LucideProps>;
    /** What dropping does ("Drop your resume here"), or what's happening ("Setting the photo…"). */
    text: string;
    /** The files it takes, in meta under the text (`md`). */
    hint?: string;
    over?: boolean;
    busy?: boolean;
    size?: "sm" | "md";
    /** Under the words (`md`): the way to choose a file instead. */
    children?: Snippet;
  }

  let {
    icon,
    text,
    hint,
    over = false,
    busy = false,
    size = "md",
    children,
    class: className,
    ...rest
  }: Props = $props();
</script>

<div class={["well", `well--${size}`, over && "well--over", className]} aria-busy={busy || undefined} {...rest}>
  <Icon {icon} weight={size === "md" ? "medium" : "regular"} />
  <p class="text">{text}</p>
  {#if hint && size === "md"}<p class="hint meta">{hint}</p>{/if}
  {#if children && size === "md"}<div class="actions">{@render children()}</div>{/if}
</div>

<style>
  .well {
    display: grid;
    justify-items: center;
    align-content: center;
    gap: var(--space-1);
    border: 1px dashed var(--input-border);
    color: var(--ink-3);
    text-align: center;
    background: var(--sunken);
    transition-property: background-color, border-color, color;
    transition-duration: var(--dur-instant);
    transition-timing-function: var(--ease-out);
  }
  /* Concentric with the popover: 4 + 8 of padding = 12. */
  .well--sm {
    height: 96px;
    border-radius: var(--radius-xs);
  }
  .well--md {
    padding: var(--space-8) var(--space-6);
    border-radius: var(--radius-lg);
  }
  .text {
    margin: 0;
    color: var(--ink-2);
    text-wrap: balance;
  }
  .well--sm .text {
    font-size: var(--text-sm);
    line-height: var(--leading-sm);
  }
  .well--md .text {
    margin-block-start: var(--space-1);
    font-weight: var(--weight-medium);
    color: var(--ink);
  }
  .hint {
    margin: 0;
    text-wrap: pretty;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-2);
    margin-block-start: var(--space-3);
  }
  .well--over {
    border-style: solid;
    border-color: var(--accent);
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  .well--over .text,
  .well--over .hint {
    color: var(--accent-text);
  }
</style>
