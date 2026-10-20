<!--
  A sidebar link: icon (or a `lead` such as a company mark), label (truncates), count. `quiet` for list entries such
  as Recent jobs (regular weight). With `sub`, a second, meta-sized line under the label (a job's role), ending in an
  optional `status` (a dot in its tone, then its word), or `busy` in its place while a model call runs for it (its
  stage word, swept as `Thinking`'s); the lead centers on both lines and each line truncates on its own, the status
  never. `trailing` draws a mark at the first line's end instead of `meta` (a job's fit `ScoreBar`).
  States: hover wash, current (pressed wash, ink icon).
-->
<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import type { HTMLAnchorAttributes } from "svelte/elements";
  import type { LucideProps } from "@lucide/svelte";
  import Icon from "./Icon.svelte";
  import { tip } from "./tip.svelte";

  interface Props extends HTMLAnchorAttributes {
    href: string;
    icon?: Component<LucideProps>;
    /** Drawn where the icon goes, for anything that isn't a Lucide icon. */
    lead?: Snippet;
    current?: boolean;
    /** A count or score, tabular, at the trailing edge of the first line. */
    meta?: string | number | null;
    /** What `meta` is, for its tooltip ("Fit score"). */
    metaTitle?: string;
    /** Drawn at the first line's end instead of `meta`. */
    trailing?: Snippet;
    /** A second line under the label. */
    sub?: string;
    /** At the end of the second line: `Applied` with an accent dot. Tones as `Badge`'s. */
    status?: { label: string; tone: "neutral" | "accent" | "gain" | "danger" } | null;
    /** A model call running for it, by its stage word ("Suggesting…"): shown in place of `status`. */
    busy?: string | null;
    quiet?: boolean;
    children: Snippet;
  }

  let {
    href,
    icon,
    lead,
    current = false,
    meta,
    metaTitle,
    trailing,
    sub,
    status,
    busy,
    quiet = false,
    children,
    class: className,
    ...rest
  }: Props = $props();
</script>

<a
  {href}
  class={["nav-item", quiet && "nav-item--quiet", sub != null && "nav-item--two", className]}
  aria-current={current ? "page" : undefined}
  {...rest}
>
  {#if lead || icon}<span class="lead"
      >{#if lead}{@render lead()}{:else if icon}<Icon {icon} weight={quiet ? "regular" : "medium"} />{/if}</span
    >{/if}
  <span class="line">
    <span class="text">{@render children()}</span>
    {#if trailing}<span class="meta">{@render trailing()}</span
      >{:else if meta != null && meta !== ""}<span class="meta num" {@attach tip(metaTitle)}>{meta}</span>{/if}
  </span>
  {#if sub != null}
    <span class="sub meta">
      <span class="text">{sub}</span>
      {#if busy}
        <span class="status sweep">{busy}</span>
      {:else if status}
        <span class="status"><span class="dot dot--{status.tone}" aria-hidden="true"></span>{status.label}</span>
      {/if}
    </span>
  {/if}
</a>

<style>
  .nav-item {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--control-md);
    padding-inline: var(--space-2);
    border-radius: var(--radius-sm);
    color: var(--ink-2);
    font-weight: var(--weight-medium);
    text-decoration: none;
    cursor: default;
    -webkit-user-drag: none;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .nav-item--quiet {
    font-weight: var(--weight-regular);
  }
  .nav-item :global(.icon) {
    color: var(--ink-3);
  }
  .nav-item[aria-current="page"] {
    color: var(--ink);
    background: var(--pressed);
  }
  .nav-item[aria-current="page"] :global(.icon) {
    color: var(--ink);
  }
  .lead {
    display: inline-flex;
    flex: none;
  }
  .line {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    flex: none;
  }
  .nav-item--two {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: var(--space-2);
    row-gap: 0;
    align-items: center;
    height: auto;
    padding-block: var(--space-1-5);
  }
  /* Centered on both lines, then 1px down: its edges meet the first line's cap height and the second line's baseline. */
  .nav-item--two > .lead {
    grid-row: span 2;
    translate: 0 1px;
  }
  .nav-item--two:not(:has(> .lead)) {
    grid-template-columns: minmax(0, 1fr);
  }
  .sub {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .status {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: var(--radius-full);
    background: var(--ink-4);
  }
  .dot--accent {
    background: var(--accent);
  }
  .dot--gain {
    background: var(--gain);
  }
  .dot--danger {
    background: var(--danger);
  }
  @media (hover: hover) {
    .nav-item:hover {
      background: var(--hover);
    }
    .nav-item[aria-current="page"]:hover {
      background: var(--pressed);
    }
  }
</style>
