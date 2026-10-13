<!--
  Renders `toast()` messages above the main area. Enters with `.enter`, exits with opacity + blur + -12px over 150ms
  (opacity only under reduced motion). Tone shows as an icon and its color, never motion alone.
-->
<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
  import X from "@lucide/svelte/icons/x";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { exit } from "./motion";
  import { dismiss, toasts } from "./toast.svelte";

  const ICONS = { gain: Check, warn: TriangleAlert, danger: CircleAlert } as const;
</script>

<div class="toaster" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class={["toast", "enter", `toast--${t.tone}`]} out:exit>
      {#if t.tone !== "neutral"}<Icon icon={ICONS[t.tone]} />{/if}
      <span class="message">{t.message}</span>
      {#if t.action}
        {@const action = t.action}
        <Button
          variant="ghost"
          size="sm"
          onclick={() => {
            dismiss(t.id);
            action.run();
          }}>{action.label}</Button
        >
      {/if}
      {#if t.tone === "danger"}
        <Button variant="ghost" size="sm" square icon={X} aria-label="Dismiss" onclick={() => dismiss(t.id)} />
      {/if}
    </div>
  {/each}
</div>

<style>
  /* Sits over the main area (the layout's main is the positioned ancestor), clear of the key footer. */
  .toaster {
    position: absolute;
    inset-inline: 0;
    bottom: var(--space-16);
    z-index: 10;
    display: grid;
    justify-items: center;
    gap: var(--space-2);
    pointer-events: none;
  }
  /* 6 inner (sm button) + 6 padding = 12 outer. */
  .toast {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    max-width: min(560px, calc(100% - 2 * var(--space-6)));
    min-height: calc(var(--control-sm) + 2 * var(--space-1-5));
    padding: var(--space-1-5) var(--space-1-5) var(--space-1-5) var(--space-3);
    border-radius: var(--radius-lg);
    border: 1px solid transparent;
    color: var(--ink);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
    pointer-events: auto;
  }
  .toast:not(:has(:global(.btn))) {
    padding-inline-end: var(--space-3);
  }
  .message {
    min-width: 0;
    text-wrap: pretty;
  }
  .toast--gain > :global(.icon) {
    color: var(--gain);
  }
  .toast--warn > :global(.icon) {
    color: var(--warn);
  }
  .toast--danger > :global(.icon) {
    color: var(--danger);
  }
</style>
