<!--
  SpeakButton: a composer's microphone (spoken practice). Off: a ghost square with the mic ("Speak your answer" and
  its key in the tooltip). Listening: the mic in accent text on an accent-soft wash and how long it's been open (m:ss,
  tabular, ticking each second); pressing it stops. Busy (macOS asking, or the last words landing): disabled.
  Unavailable (`off`, why): still focusable (`aria-disabled`), its tooltip says why. No motion: the wash, the color
  and the time say it's open.
-->
<script lang="ts">
  import Mic from "@lucide/svelte/icons/mic";
  import Button from "./Button.svelte";
  import { tip } from "./tip.svelte";

  interface Props {
    /** When the microphone opened (`performance.now()` clock); `null` while it's closed. */
    openedAt: number | null;
    busy?: boolean;
    /** Why it can't open here; it stays focusable so the tooltip can say so. */
    off?: string | null;
    /** The key that does the same thing, named in the tooltip. */
    kbd?: string;
    onclick: () => void;
  }

  let { openedAt, busy = false, off = null, kbd, onclick }: Props = $props();

  let now = $state(performance.now());
  $effect(() => {
    if (openedAt === null) return;
    now = performance.now();
    const timer = setInterval(() => (now = performance.now()), 250);
    return () => clearInterval(timer);
  });

  const seconds = $derived(openedAt === null ? 0 : Math.max(0, Math.floor((now - openedAt) / 1000)));
  const time = $derived(`${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`);
  const key = $derived(kbd ? ` (${kbd})` : "");
</script>

{#if openedAt !== null}
  <Button
    variant="ghost"
    size="sm"
    icon={Mic}
    class="speak speak--on"
    aria-label={`Stop speaking, ${seconds} seconds`}
    aria-pressed="true"
    disabled={busy}
    {onclick}
    {@attach tip(`Stop${key}`)}><span class="num">{time}</span></Button
  >
{:else}
  <Button
    variant="ghost"
    size="sm"
    square
    icon={Mic}
    class="speak"
    aria-label="Speak your answer"
    aria-pressed="false"
    aria-disabled={off ? "true" : undefined}
    disabled={busy}
    onclick={() => (off ? undefined : onclick())}
    static={!!off}
    {@attach tip(off ?? `Speak your answer${key}`)}
  />
{/if}

<style>
  /* Listening: accent text on the accent wash, as a selected thing reads. */
  :global(.btn.speak--on) {
    color: var(--accent-text);
    background: var(--accent-soft);
  }
  @media (hover: hover) {
    :global(.btn.speak--on:not(:disabled):hover) {
      color: var(--accent-text);
      background: var(--accent-soft);
    }
  }
  :global(.btn.speak[aria-disabled="true"]) {
    color: var(--ink-4);
  }
  @media (hover: hover) {
    :global(.btn.speak[aria-disabled="true"]:hover) {
      color: var(--ink-4);
      background: transparent;
    }
  }
</style>
