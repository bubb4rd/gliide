// One keydown dispatcher for the whole window. Screens register bindings while mounted:
//
//   $effect(() => bindKeys({ ArrowDown: next, " ": toggle, Enter: edit, a: add, "mod+z": undo, "shift+mod+z": redo }));
//
// A binding is a key (`KeyboardEvent.key`; letters in lowercase) with optional `shift+`, `alt+`, `ctrl+` and `mod+`
// (⌘ on macOS, Ctrl elsewhere), in any order. The newest bindings are asked first and older ones catch what they
// don't bind. While you type in a field only ⌘-chords reach the bindings. A handler that returns `false` lets the
// key through (no preventDefault, older bindings and the default action still get it).

export type KeyHandler = (event: KeyboardEvent) => unknown;
export type KeyBindings = Record<string, KeyHandler>;

const MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
const MODIFIERS = ["shift", "alt", "ctrl", "mod"] as const;

/** A binding in one spelling: modifiers in a fixed order, then the key. */
function canonical(binding: string): string {
  const parts = binding === "+" ? ["+"] : binding.split("+").map((p) => (p === "" ? "+" : p));
  const key = parts.pop()!;
  const mods = new Set(parts.map((p) => p.toLowerCase()));
  return [...MODIFIERS.filter((m) => mods.has(m)), key.length === 1 ? key.toLowerCase() : key].join("+");
}

/** The event in the same spelling. Shift counts only with letters and named keys (`?` already implies it). */
function spell(e: KeyboardEvent): string {
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  const letter = /^[a-z]$/.test(key);
  const mods: string[] = [];
  if (e.shiftKey && (letter || e.key.length > 1)) mods.push("shift");
  if (e.altKey) mods.push("alt");
  if (MAC ? e.ctrlKey : false) mods.push("ctrl");
  if (MAC ? e.metaKey : e.ctrlKey) mods.push("mod");
  return [...mods, key].join("+");
}

function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || target.matches("input:not([type=checkbox], [type=radio]), textarea, select");
}

const layers: Map<string, KeyHandler>[] = [];

function dispatch(e: KeyboardEvent) {
  if (e.defaultPrevented || e.isComposing) return;
  const combo = spell(e);
  if (typing(e.target) && !combo.includes("mod+")) return;
  for (let i = layers.length - 1; i >= 0; i--) {
    const handler = layers[i].get(combo);
    if (!handler) continue;
    if (handler(e) === false) continue;
    e.preventDefault();
    return;
  }
}

/** Registers `bindings` until the returned function is called (an `$effect` cleanup). */
export function bindKeys(bindings: KeyBindings): () => void {
  const layer = new Map(Object.entries(bindings).map(([k, h]) => [canonical(k), h]));
  if (!layers.length) window.addEventListener("keydown", dispatch);
  layers.push(layer);
  return () => {
    const i = layers.indexOf(layer);
    if (i >= 0) layers.splice(i, 1);
    if (!layers.length) window.removeEventListener("keydown", dispatch);
  };
}
