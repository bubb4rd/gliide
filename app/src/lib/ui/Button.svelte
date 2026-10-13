<!--
  Button: default (surface + shadow ring), primary (the screen's one action), ghost (toolbars, row actions), danger.
  Presses scale to 0.96 (never when disabled, never under reduced motion); `static` opts out.
-->
<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import type { LucideProps } from "@lucide/svelte";
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";

  interface Props extends HTMLButtonAttributes {
    variant?: "default" | "primary" | "ghost" | "danger";
    size?: "sm" | "md" | "lg";
    /** Icon only, square; give it `aria-label`. */
    square?: boolean;
    icon?: Component<LucideProps>;
    iconEnd?: Component<LucideProps>;
    /** The key that does the same thing, shown inside the button. */
    kbd?: string;
    /** No press scale, where the motion would distract. */
    static?: boolean;
    element?: HTMLButtonElement;
    children?: Snippet;
  }

  let {
    variant = "default",
    size = "md",
    square = false,
    icon,
    iconEnd,
    kbd,
    static: still = false,
    type = "button",
    element = $bindable(),
    children,
    class: className,
    ...rest
  }: Props = $props();
</script>

<button
  bind:this={element}
  {type}
  class={[
    "btn",
    variant !== "default" && `btn--${variant}`,
    size !== "md" && `btn--${size}`,
    square && "btn--square",
    icon && !square && children && "btn--icon-start",
    (iconEnd || kbd) && !square && "btn--icon-end",
    still && "btn--static",
    className,
  ]}
  {...rest}
>
  {#if icon}<Icon {icon} size={size === "sm" ? 14 : 16} weight="medium" />{/if}
  {@render children?.()}
  {#if iconEnd}<Icon icon={iconEnd} size={14} weight="medium" />{/if}
  {#if kbd}<Kbd>{kbd}</Kbd>{/if}
</button>

<style>
  .btn {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    gap: var(--space-1-5);
    height: var(--control-md);
    padding-inline: var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    font: var(--weight-medium) var(--text-base) / 1 var(--font-ui);
    letter-spacing: var(--tracking-base);
    white-space: nowrap;
    color: var(--ink);
    background: var(--surface);
    box-shadow: var(--shadow-border);
    cursor: default;
    user-select: none;
    transition-property: scale, box-shadow, background-color;
    transition-duration: var(--dur-fast);
    transition-timing-function: var(--ease-out);
  }
  @media (prefers-reduced-motion: no-preference) {
    .btn:not(:disabled):not(.btn--static):active {
      scale: var(--press-scale);
    }
  }
  .btn:disabled {
    color: var(--ink-4);
    box-shadow: none;
    background: var(--sunken);
  }
  .btn :global(.kbd) {
    margin-inline-end: calc(var(--space-1) * -1);
    height: 16px;
    min-width: 16px;
  }

  /* Optical alignment: 2px less padding on the icon side. */
  .btn--icon-start {
    padding-inline-start: calc(var(--space-3) - 2px);
  }
  .btn--icon-end {
    padding-inline-end: calc(var(--space-3) - 2px);
  }

  .btn--primary {
    color: var(--on-accent);
    background: var(--accent);
    box-shadow: none;
  }
  .btn--primary :global(.kbd) {
    color: var(--on-accent);
    background: oklch(from var(--on-accent) l c h / 0.16);
    box-shadow: none;
  }
  .btn--primary:disabled {
    color: var(--ink-4);
    background: var(--sunken);
  }
  .btn--primary:disabled :global(.kbd) {
    color: var(--ink-4);
  }
  .btn--ghost {
    background: transparent;
    box-shadow: none;
    color: var(--ink-2);
  }
  .btn--ghost:disabled {
    background: transparent;
  }
  .btn--danger {
    color: var(--danger);
  }
  .btn--sm {
    height: var(--control-sm);
    padding-inline: var(--space-2);
    font-size: var(--text-sm);
    gap: var(--space-1);
  }
  .btn--sm.btn--icon-start {
    padding-inline-start: calc(var(--space-2) - 2px);
  }
  .btn--sm.btn--icon-end {
    padding-inline-end: calc(var(--space-2) - 2px);
  }
  .btn--lg {
    height: var(--control-lg);
    padding-inline: var(--space-4);
    font-size: var(--text-md);
  }
  .btn--lg.btn--icon-start {
    padding-inline-start: calc(var(--space-4) - 2px);
  }
  .btn--lg.btn--icon-end {
    padding-inline-end: calc(var(--space-4) - 2px);
  }
  .btn--square {
    width: var(--control-md);
    padding: 0;
  }
  .btn--sm.btn--square {
    width: var(--control-sm);
  }
  .btn--lg.btn--square {
    width: var(--control-lg);
  }

  @media (hover: hover) {
    .btn:not(:disabled):hover {
      box-shadow: var(--shadow-border-hover);
    }
    .btn--primary:not(:disabled):hover {
      background: oklch(from var(--accent) calc(l - 0.04) c h);
      box-shadow: none;
    }
    .btn--ghost:not(:disabled):hover {
      background: var(--hover);
      color: var(--ink);
      box-shadow: none;
    }
  }
</style>
