<!--
  A sidebar link: icon, label (truncates), count. `quiet` for list entries such as Recent jobs (regular weight).
  States: hover wash, current (pressed wash, ink icon).
-->
<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import type { LucideProps } from "@lucide/svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    href: string;
    icon?: Component<LucideProps>;
    current?: boolean;
    /** A count or score, tabular, at the trailing edge. */
    meta?: string | number | null;
    quiet?: boolean;
    title?: string;
    children: Snippet;
  }

  let { href, icon, current = false, meta, quiet = false, title, children }: Props = $props();
</script>

<a {href} {title} class={["nav-item", quiet && "nav-item--quiet"]} aria-current={current ? "page" : undefined}>
  {#if icon}<Icon {icon} weight={quiet ? "regular" : "medium"} />{/if}
  <span class="text">{@render children()}</span>
  {#if meta != null && meta !== ""}<span class="meta num">{meta}</span>{/if}
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
  @media (hover: hover) {
    .nav-item:hover {
      background: var(--hover);
    }
    .nav-item[aria-current="page"]:hover {
      background: var(--pressed);
    }
  }
</style>
