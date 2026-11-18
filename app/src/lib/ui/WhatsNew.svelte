<!--
  WhatsNew: a release's notes, once, on the first launch of a new version. A 320px floating card (`--surface`,
  `--radius-lg`, `--shadow-popover`, no scrim); where it floats is its parent's call (the layout: the window's lower
  right). Its top is the release's picture, as painted, bleeding to the card's edges and fading into the card over its
  lower 60%, with "What’s new in 0.11.0" (serif `.title`) and "Updated Nov 17" (ink 2) set on the faded part, so they
  read in both themes on any picture. Without a picture, the app icon (44) beside the same title. Then the notes as
  `ReleaseNotes`' list, folded to the first two with "Show 5 more" (a ghost toggle, `aria-expanded`; three or fewer all
  show, no toggle); unfolded, the card grows to 560px at most (`--wn-room` caps it to the window), and past that the
  notes scroll inside, a hairline under the picture once scrolled and over the foot while more is below. Foot: **Release
  on GitHub** (ghost, arrow up-right) and **Got it** `Esc` (default, never the accent). × (square, `sm`; on its own
  surface over a picture, ghost without one) also closes. Enters as `.enter`; leaves with `exit`. Never takes focus; the
  caller binds Esc.
-->
<script lang="ts">
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronUp from "@lucide/svelte/icons/chevron-up";
  import X from "@lucide/svelte/icons/x";
  import Button from "./Button.svelte";
  import { exit } from "./motion";
  import ReleaseNotes, { releaseChanges } from "./ReleaseNotes.svelte";
  import { tip } from "./tip.svelte";

  interface Props {
    version: string;
    /** When the release was published (RFC 3339). */
    date: string | null;
    notes: string;
    /** The release's picture (a blob or file URL); `null`: the app icon instead. */
    picture: string | null;
    /** Opens the release on GitHub. */
    onrelease: () => void;
    /** Got it, ×, or the caller's Esc. */
    ondismiss: () => void;
  }

  let { version, date, notes, picture, onrelease, ondismiss }: Props = $props();

  /** Folded, the release's headliners: its first changes. */
  const FOLD = 2;
  const uid = $props.id();
  const count = $derived(releaseChanges(notes).length);
  /** Three or fewer show whole: hiding one behind "Show 1 more" says less than showing it. */
  const folds = $derived(count > FOLD + 1);
  let open = $state(false);
  const day = $derived(
    date ? new Date(date).toLocaleDateString(undefined, { month: "short", day: "numeric" }) : null,
  );

  // The notes scroll once the card is at its height: a hairline under the picture once scrolled, over the foot while
  // more is below.
  let body = $state<HTMLElement>();
  let scrolled = $state(false);
  let more = $state(false);
  function measure() {
    if (!body) return;
    scrolled = body.scrollTop > 0;
    more = body.scrollHeight - body.scrollTop - body.clientHeight > 1;
  }
  $effect(() => {
    if (!body) return;
    void open;
    const watch = new ResizeObserver(measure);
    watch.observe(body);
    return () => watch.disconnect();
  });
</script>

<aside
  class={["wn", "enter", !picture && "wn--plain", scrolled && "wn--scrolled", more && "wn--more"]}
  aria-labelledby="{uid}-title"
  out:exit|global
>
  <div class="hero">
    {#if picture}
      <img class="picture" src={picture} alt="" draggable="false" />
    {:else}
      <img class="icon" src="/icon.png" alt="" width="44" height="44" draggable="false" />
    {/if}
    <div class="head">
      <h2 class="title" id="{uid}-title">What’s new in {version}</h2>
      {#if day && date}<p class="sub">Updated <time class="num" datetime={date}>{day}</time></p>{/if}
    </div>
  </div>
  <div class="close">
    <Button
      variant={picture ? "default" : "ghost"}
      size="sm"
      square
      icon={X}
      aria-label="Close What’s new"
      {@attach tip("Close (Esc)")}
      onclick={ondismiss}
    />
  </div>
  <div class="body" bind:this={body} onscroll={measure}>
    <ReleaseNotes {notes} limit={folds && !open ? FOLD : null} id="{uid}-notes" />
    {#if folds}
      <div class="fold">
        <Button
          variant="ghost"
          size="sm"
          icon={open ? ChevronUp : ChevronDown}
          aria-expanded={open}
          aria-controls="{uid}-notes"
          onclick={() => (open = !open)}>{open ? "Show fewer" : `Show ${count - FOLD} more`}</Button
        >
      </div>
    {/if}
  </div>
  <footer class="foot">
    <Button variant="ghost" size="sm" iconEnd={ArrowUpRight} onclick={onrelease}>Release on GitHub</Button>
    <Button size="sm" kbd="Esc" onclick={ondismiss}>Got it</Button>
  </footer>
</aside>

<style>
  .wn {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 320px;
    max-height: min(560px, var(--wn-room, 560px));
    overflow: hidden;
    isolation: isolate;
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    color: var(--ink);
    background: var(--surface);
    box-shadow: var(--shadow-popover);
  }

  /* The picture: 168px tall (a release ships it at 640×336), as painted, fading into the card over its lower 60%
     (a smootherstep from opaque at 30% to clear at 90%). */
  .hero {
    position: relative;
    flex: none;
    display: flex;
    align-items: flex-end;
    min-height: 168px;
  }
  .picture {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    pointer-events: none;
    user-select: none;
    /* A mask reads alpha alone: the stops are ink at the curve's opacities. */
    mask-image: linear-gradient(
      to bottom,
      var(--ink) 30%,
      oklch(from var(--ink) l c h / 0.991) 36%,
      oklch(from var(--ink) l c h / 0.942) 42%,
      oklch(from var(--ink) l c h / 0.837) 48%,
      oklch(from var(--ink) l c h / 0.683) 54%,
      oklch(from var(--ink) l c h / 0.5) 60%,
      oklch(from var(--ink) l c h / 0.317) 66%,
      oklch(from var(--ink) l c h / 0.163) 72%,
      oklch(from var(--ink) l c h / 0.058) 78%,
      oklch(from var(--ink) l c h / 0.009) 84%,
      transparent 90%
    );
  }
  .head {
    position: relative;
    display: grid;
    padding: 0 var(--space-12) var(--space-1) var(--space-4);
  }
  .title {
    margin: 0;
  }
  .sub {
    margin: 0;
    color: var(--ink-2);
  }

  /* No picture: the app icon (as published) beside the title. */
  .wn--plain .hero {
    min-height: 0;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-12) var(--space-1) var(--space-4);
  }
  .wn--plain .head {
    padding: 0;
  }
  .icon {
    flex: none;
    margin: calc(var(--space-1) * -1);
  }

  /* 6px in from the corner: 6 + 6 = 12, concentric with the card. */
  .close {
    position: absolute;
    z-index: 1;
    inset-block-start: var(--space-1-5);
    inset-inline-end: var(--space-1-5);
  }

  .body {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    display: grid;
    align-content: start;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4) var(--space-1);
  }
  .wn--scrolled .body {
    box-shadow: inset 0 1px 0 var(--line);
  }
  /* The toggle's chevron takes the dots' column, so its words line up with the changes'. */
  .fold {
    margin-inline-start: calc(var(--space-2) * -1);
  }

  .foot {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-3) var(--space-3) var(--space-2);
  }
  .wn--more .foot {
    box-shadow: inset 0 1px 0 var(--line);
  }

  @media (forced-colors: active) {
    .picture {
      display: none;
    }
  }
</style>
