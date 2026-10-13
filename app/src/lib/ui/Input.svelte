<!-- Single-line field: --input-border edge (3:1), accent border + 3px ring on focus. -->
<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";

  interface Props extends Omit<HTMLInputAttributes, "size" | "value"> {
    value?: string;
    /** Height: 24, 28, or 36 (the default; DESIGN.md puts inputs at --control-lg). */
    size?: "sm" | "md" | "lg";
  }

  let { value = $bindable(""), size = "lg", type = "text", class: className, ...rest }: Props = $props();

  let element: HTMLInputElement;

  export function focus(options?: FocusOptions) {
    element.focus(options);
  }

  export function select() {
    element.select();
  }
</script>

<input bind:this={element} bind:value {type} class={["input", `input--${size}`, className]} {...rest} />

<style>
  .input {
    width: 100%;
    min-width: 0;
    padding-inline: var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--input-border);
    background: var(--surface);
    color: var(--ink);
    font: var(--weight-regular) var(--text-base) / var(--leading-base) var(--font-ui);
    letter-spacing: var(--tracking-base);
    transition-property: border-color, box-shadow;
    transition-duration: var(--dur-fast);
    transition-timing-function: var(--ease-out);
  }
  .input--lg {
    height: var(--control-lg);
  }
  .input--md {
    height: var(--control-md);
    padding-inline: var(--space-2-5);
  }
  .input--sm {
    height: var(--control-sm);
    padding-inline: var(--space-2);
    font-size: var(--text-sm);
  }
  .input::placeholder {
    color: var(--ink-3);
  }
  .input:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px oklch(from var(--accent) l c h / 0.18);
  }
  .input:disabled {
    color: var(--ink-4);
    background: var(--sunken);
    border-color: var(--line-strong);
  }
</style>
