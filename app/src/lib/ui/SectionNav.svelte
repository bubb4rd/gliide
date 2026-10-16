<!--
  SectionNav: quick ways to the sections of a long screen (Settings). A link per section, a `sub` one indented under
  the one before it in regular weight; the one you're reading is ink, with a 2px accent tick at its leading edge. It
  follows `root`'s scroll: a section is current once its top reaches its scroll margin, the last one once the end is
  reached, and the one you jumped to for as long as its top stays in view (so a short last section can be current). A
  link jumps at once, no smooth scroll, and moves focus to its section (give it `tabindex="-1"` and no focus ring:
  the current link marks where you are). A column; under
  720px of its container (the nearest with `container-type: inline-size`) a row over a hairline, every link alike, the
  tick on the hairline under the current one, scrolling sideways when the links don't fit.
-->
<script lang="ts" module>
  export interface NavSection {
    /** The id of the element it jumps to. */
    id: string;
    label: string;
    /** A part of the section before it, indented under it. */
    sub?: boolean;
  }
</script>

<script lang="ts">
  interface Props {
    sections: NavSection[];
    /** The element whose scroll moves through the sections. */
    root: HTMLElement | undefined;
    /** Names the landmark ("Settings sections"). */
    label: string;
  }

  let { sections, root, label }: Props = $props();

  /** How far past its scroll margin a section's top may sit and still count as reached. */
  const REACH = 24;

  let nav = $state<HTMLElement>();
  let current = $state<string | null>(null);
  /** The section last jumped to: current while its top stays in view. Cleared once it leaves. */
  let jumped: string | null = null;

  const marginOf = (el: Element) => parseFloat(getComputedStyle(el).scrollMarginTop) || 0;

  function update() {
    if (!root) return;
    const view = root.getBoundingClientRect();
    const targets = sections.flatMap((s) => {
      const el = document.getElementById(s.id);
      return el ? [{ id: s.id, el }] : [];
    });
    if (!targets.length) {
      current = null;
      return;
    }
    if (jumped) {
      const top = document.getElementById(jumped)?.getBoundingClientRect().top;
      if (top !== undefined && top >= view.top && top < view.bottom) {
        current = jumped;
        return;
      }
      jumped = null;
    }
    if (root.scrollTop > 0 && root.scrollTop + root.clientHeight >= root.scrollHeight - 1) {
      current = targets[targets.length - 1].id;
      return;
    }
    let at = targets[0].id;
    for (const { id, el } of targets) {
      if (el.getBoundingClientRect().top > view.top + marginOf(el) + REACH) break;
      at = id;
    }
    current = at;
  }

  $effect(() => {
    const scroller = root;
    if (!scroller) return;
    update();
    // Sections grow and shrink (a list opening, a card arriving, a card redrawn) without a scroll.
    const resize = new ResizeObserver(() => update());
    resize.observe(scroller);
    for (const child of scroller.children) resize.observe(child);
    scroller.addEventListener("scroll", update, { passive: true });
    return () => {
      resize.disconnect();
      scroller.removeEventListener("scroll", update);
    };
  });

  // As a row that doesn't fit: the current link scrolled into it.
  $effect(() => {
    if (!nav || !current || nav.scrollWidth <= nav.clientWidth) return;
    const link = nav.querySelector<HTMLElement>('[aria-current="location"]');
    if (!link) return;
    const start = link.offsetLeft;
    const end = start + link.offsetWidth;
    if (start < nav.scrollLeft) nav.scrollLeft = start;
    else if (end > nav.scrollLeft + nav.clientWidth) nav.scrollLeft = end - nav.clientWidth;
  });

  /** Scrolls `root` alone (never the page around it) to put the section's top at its scroll margin. */
  function jump(e: MouseEvent, id: string) {
    const target = document.getElementById(id);
    if (!target || !root) return;
    e.preventDefault();
    jumped = id;
    current = id;
    const offset = target.getBoundingClientRect().top - root.getBoundingClientRect().top;
    root.scrollTo({ top: root.scrollTop + offset - marginOf(target) });
    target.focus({ preventScroll: true });
  }
</script>

<nav class="section-nav" aria-label={label} bind:this={nav}>
  {#each sections as section (section.id)}
    <a
      href="#{section.id}"
      class={["item", section.sub && "item--sub"]}
      aria-current={current === section.id ? "location" : undefined}
      onclick={(e) => jump(e, section.id)}>{section.label}</a
    >
  {/each}
</nav>

<style>
  .section-nav {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 1px;
  }
  .item {
    position: relative;
    display: block;
    height: var(--control-md);
    padding-inline: var(--space-3);
    overflow: hidden;
    border-radius: var(--radius-sm);
    line-height: var(--control-md);
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--ink-3);
    font-weight: var(--weight-medium);
    text-decoration: none;
    cursor: default;
    -webkit-user-drag: none;
    transition-property: background-color, color;
    transition-duration: var(--dur-instant);
    transition-timing-function: var(--ease-out);
  }
  .item--sub {
    padding-inline-start: var(--space-6);
    font-weight: var(--weight-regular);
  }
  .item[aria-current="location"] {
    color: var(--ink);
  }
  .item[aria-current="location"]::before {
    content: "";
    position: absolute;
    inset-block: var(--space-1-5);
    inset-inline-start: 0;
    width: 2px;
    border-radius: var(--radius-full);
    background: var(--accent);
  }
  @media (hover: hover) {
    .item:hover {
      color: var(--ink-2);
      background: var(--hover);
    }
    .item[aria-current="location"]:hover {
      color: var(--ink);
    }
  }
  @container (width < 720px) {
    .section-nav {
      display: flex;
      gap: var(--space-1);
      padding-bottom: var(--space-2);
      overflow-x: auto;
      overscroll-behavior-x: contain;
      scrollbar-width: none;
      box-shadow: inset 0 -1px 0 var(--line);
    }
    .item {
      flex: none;
      overflow: visible;
    }
    .item--sub {
      padding-inline-start: var(--space-3);
      font-weight: var(--weight-medium);
    }
    /* On the hairline under the link. */
    .item[aria-current="location"]::before {
      inset-block: auto calc(var(--space-2) * -1);
      inset-inline: var(--space-3);
      width: auto;
      height: 2px;
    }
  }
  @media (forced-colors: active) {
    .item[aria-current="location"]::before {
      forced-color-adjust: none;
      background: Highlight;
    }
  }
</style>
