<!--
  Multi-line field. `size="lg"` sets the reading size (14/22), so an inline edit of a resume line keeps the row's
  type and doesn't reflow. Grows with its content where the engine supports `field-sizing`.
-->
<script lang="ts">
  import type { HTMLTextareaAttributes } from "svelte/elements";

  interface Props extends Omit<HTMLTextareaAttributes, "value"> {
    value?: string;
    size?: "md" | "lg";
  }

  let { value = $bindable(""), size = "md", class: className, ...rest }: Props = $props();

  let element: HTMLTextAreaElement;

  export function focus(options?: FocusOptions) {
    element.focus(options);
  }

  export function select() {
    element.select();
  }
</script>

<textarea bind:this={element} bind:value class={["textarea", `textarea--${size}`, className]} {...rest}></textarea>

<style>
  .textarea {
    display: block;
    width: 100%;
    min-height: 96px;
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--input-border);
    background: var(--surface);
    color: var(--ink);
    font: var(--weight-regular) var(--text-base) / var(--leading-base) var(--font-ui);
    letter-spacing: var(--tracking-base);
    resize: vertical;
    field-sizing: content;
    transition-property: border-color, box-shadow;
    transition-duration: var(--dur-fast);
    transition-timing-function: var(--ease-out);
  }
  .textarea--lg {
    min-height: 0;
    font-size: var(--text-md);
    line-height: var(--leading-md);
    letter-spacing: var(--tracking-md);
    text-wrap: pretty;
  }
  .textarea::placeholder {
    color: var(--ink-3);
  }
  .textarea:focus-visible {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px oklch(from var(--accent) l c h / 0.18);
  }
  .textarea:disabled {
    color: var(--ink-4);
    background: var(--sunken);
    border-color: var(--line-strong);
  }
</style>
