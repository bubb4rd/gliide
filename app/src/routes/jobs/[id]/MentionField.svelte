<!--
  A field for an ask whose parts sit in its words: what you type, and each part you point at (an entry, a skills line,
  the summary, a quoted line) as a `MentionChip` where you put it. A chip is one piece of the text: the caret steps
  over it, Backspace takes it whole, ⌘Z brings it back. Paste and drops come in as plain text, ⇧⏎ breaks the line,
  formatting keys do nothing; `placeholder` while it's empty. Grows with its text (its owner caps the height).

  The owner gets a `Draft` on every change (`oninput`): the words with each part's token where it sits (`@elovatesr.com`,
  “the line”), and the parts with where (`at`). It puts parts and text in at the caret (`put`), reads the words before
  the caret (`before`, for `@` and `"` menus), and fills or empties it (`set`, `clear`).
-->
<script lang="ts">
  import { mount, onDestroy, onMount, unmount } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";
  import MentionChip from "./MentionChip.svelte";
  import { pieces, token, type Draft, type Mention } from "./thread";

  interface Props extends Omit<HTMLAttributes<HTMLDivElement>, "oninput" | "onkeydown" | "placeholder"> {
    /** Its accessible name. */
    label: string;
    placeholder?: string;
    disabled?: boolean;
    /** Its content when it mounts. */
    initial?: Draft;
    /** Focused when it mounts, the caret at the end. */
    autofocus?: boolean;
    oninput?: (draft: Draft) => void;
    /** Runs first; ⇧⏎ breaks the line unless it prevented the default. */
    onkeydown?: (e: KeyboardEvent) => void;
  }

  let {
    label,
    placeholder,
    initial,
    autofocus = false,
    disabled = false,
    oninput,
    onkeydown,
    class: className,
    ...rest
  }: Props = $props();

  let root = $state<HTMLDivElement>();
  let empty = $state(true);
  /** Each chip's part, by the element holding it (the browser may take it out and, on ⌘Z, put the same one back). */
  const chips = new WeakMap<Element, Mention>();
  /** Every chip mounted since the last `set` or `clear`, unmounted then. */
  const mounted: ReturnType<typeof mount>[] = [];
  let seq = 0;

  function chip(el: HTMLElement, m: Mention) {
    const { at: _, ...part } = m;
    el.textContent = "";
    el.contentEditable = "false";
    chips.set(el, part);
    mounted.push(mount(MentionChip, { target: el, props: { label: part.label, quote: part.quote } }));
  }

  function read(): Draft {
    let text = "";
    const scope: Mention[] = [];
    const walk = (node: Node) => {
      for (const c of node.childNodes) {
        if (c instanceof Text) text += c.data;
        else if (c instanceof HTMLElement) {
          const m = chips.get(c);
          if (m) {
            if (!scope.some((s) => s.id === m.id)) scope.push({ ...m, at: text.length });
            text += token(m);
          } else if (c.tagName === "BR") text += "\n";
          else {
            // A line the browser put in a block of its own.
            if ((c.tagName === "DIV" || c.tagName === "P") && text && !text.endsWith("\n")) text += "\n";
            walk(c);
          }
        }
      }
    };
    if (root) walk(root);
    // A browser's no-break spaces are spaces; its trailing <br> holds the last line open, no more.
    return { text: text.replace(/\u00a0/g, " ").replace(/\n$/, ""), scope };
  }

  function changed() {
    const draft = read();
    empty = draft.text === "";
    // What a browser leaves in a field you emptied (a <br>) would keep the placeholder away.
    if (empty && root?.childNodes.length) root.replaceChildren();
    oninput?.(draft);
  }

  function reset() {
    for (const c of mounted.splice(0)) void unmount(c);
    root?.replaceChildren();
  }

  function caretInside(): Selection | null {
    const sel = getSelection();
    return root && sel?.anchorNode && root.contains(sel.anchorNode) ? sel : null;
  }

  /** Focuses it, the caret at the end with `end`, else where it was (at the end when it wasn't in it). */
  export function focus(end = false) {
    if (!root || disabled) return;
    const kept = !end && caretInside();
    root.focus();
    if (kept) return;
    const range = document.createRange();
    range.selectNodeContents(root);
    range.collapse(false);
    const sel = getSelection();
    sel?.removeAllRanges();
    sel?.addRange(range);
  }

  /** Its content as `draft` says (not undoable). */
  export function set(draft: Draft) {
    if (!root) return;
    reset();
    for (const p of pieces(draft.text, draft.scope)) {
      if (typeof p === "string") root.append(p);
      else {
        const el = document.createElement("span");
        root.append(el);
        chip(el, p);
      }
    }
    changed();
  }

  export function clear() {
    reset();
    changed();
  }

  /** The words between the caret and the part or line start before it; null without a caret in it. */
  export function before(): string | null {
    const sel = caretInside();
    if (!sel?.isCollapsed) return null;
    let node: Node | null = sel.anchorNode;
    let text = "";
    if (node instanceof Text) {
      text = node.data.slice(0, sel.anchorOffset);
      node = node.previousSibling;
    } else node = node?.childNodes[sel.anchorOffset - 1] ?? null;
    for (; node instanceof Text; node = node.previousSibling) text = node.data + text;
    return text.replace(/\u00a0/g, " ");
  }

  /** The character just after the caret: "" at the end or before a part, "\n" before a line break. */
  function next(): string {
    const sel = caretInside();
    const node = sel?.anchorNode;
    if (!sel || !node) return "";
    if (node instanceof Text && sel.anchorOffset < node.data.length) return node.data[sel.anchorOffset];
    const after = node instanceof Text ? node.nextSibling : node.childNodes[sel.anchorOffset];
    if (after instanceof Text) return after.data[0] ?? "";
    return after instanceof HTMLBRElement ? "\n" : "";
  }

  /**
   * Puts `content` in at the caret (focusing it first), in place of the `replacing` characters before it: text as
   * typed, or a part as its chip with a space after it (unless one follows). Undoable, as typing is.
   */
  export function put(content: Mention | string, replacing = 0) {
    if (!root || disabled) return;
    focus();
    const sel = getSelection();
    for (let i = 0; i < replacing; i++) sel?.modify("extend", "backward", "character");
    if (typeof content === "string") {
      document.execCommand("insertText", false, content);
      return;
    }
    // A holder with a character in it (an empty one may not survive the insert); the chip takes its place.
    const key = `chip-${++seq}`;
    const holder = `<span data-chip="${key}">@</span>`;
    document.execCommand("insertHTML", false, holder + (/^\s/.test(next()) ? "" : "&nbsp;"));
    const el = root.querySelector<HTMLElement>(`[data-chip="${key}"]`);
    if (el) chip(el, content);
    changed();
  }

  function keydown(e: KeyboardEvent) {
    onkeydown?.(e);
    if (e.defaultPrevented || e.isComposing) return;
    if (e.key === "Enter") {
      e.preventDefault();
      document.execCommand("insertLineBreak");
    }
  }

  function beforeinput(e: InputEvent) {
    if (e.inputType.startsWith("format")) e.preventDefault();
  }

  function paste(e: ClipboardEvent) {
    e.preventDefault();
    const text = e.clipboardData?.getData("text/plain");
    if (text) document.execCommand("insertText", false, text);
  }

  function drop(e: DragEvent) {
    e.preventDefault();
    const text = e.dataTransfer?.getData("text/plain");
    const at = document.caretRangeFromPoint?.(e.clientX, e.clientY);
    if (!text || !at || !root?.contains(at.startContainer)) return;
    root.focus();
    const sel = getSelection();
    sel?.removeAllRanges();
    sel?.addRange(at);
    document.execCommand("insertText", false, text);
  }

  onMount(() => {
    if (initial) set(initial);
    if (autofocus) focus(true);
  });
  onDestroy(reset);
</script>

<div
  bind:this={root}
  {...rest}
  class={["field", empty && "field--empty", disabled && "field--disabled", className]}
  contenteditable={!disabled}
  role="textbox"
  tabindex={disabled ? -1 : 0}
  aria-multiline="true"
  aria-label={label}
  aria-placeholder={placeholder}
  aria-disabled={disabled || undefined}
  data-placeholder={placeholder}
  onkeydown={keydown}
  onbeforeinput={beforeinput}
  oninput={changed}
  onpaste={paste}
  ondrop={drop}
></div>

<style>
  /* The reading size, as `Textarea size="lg"`; spaces and line breaks kept as typed. */
  .field {
    position: relative;
    min-width: 0;
    color: var(--ink);
    font: var(--weight-regular) var(--text-md) / var(--leading-md) var(--font-ui);
    letter-spacing: var(--tracking-md);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    outline: none;
    cursor: text;
  }
  /* Laid over the caret's line, so the caret sits where your first word goes. */
  .field--empty::before {
    content: attr(data-placeholder);
    position: absolute;
    inset-inline: 0;
    padding-inline: inherit;
    overflow: hidden;
    color: var(--ink-3);
    text-overflow: ellipsis;
    white-space: nowrap;
    pointer-events: none;
  }
  .field--disabled {
    color: var(--ink-4);
    cursor: default;
  }
</style>
