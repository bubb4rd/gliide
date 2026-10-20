// Finding the page's line you mean from words you type after a `"`: quoted as written, cut short, or paraphrased.
// A line scores for holding the words as typed, then for each word it shares: whole, as the same stem
// ("collaborated" for "collaborating"), begun (the word you're still typing) or sharing a root ("design" for
// "designers"). Small words don't count.

const SMALL: Record<string, true> = Object.fromEntries(
  ["a", "an", "the", "and", "or", "of", "to", "in", "on", "for", "with", "by", "at", "as", "my", "i"].map((w) => [w, true]),
);
/** The most lines the menu lists for words typed. */
const MOST = 8;

const words = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .match(/[a-z0-9+#]+/g) ?? [];
const stem = (w: string) => (w.length > 4 ? w.replace(/(?:ing|ed|es|s)$/, "") : w);

/** How well `line` answers `query` (0: not at all). */
export function score(query: string, line: string): number {
  const typed = query.trim().toLowerCase();
  if (!typed) return 0;
  const text = line.toLowerCase();
  const s = text.includes(typed) ? 2 + (text.startsWith(typed) ? 0.5 : 0) : 0;
  const asked = words(typed).filter((w) => SMALL[w] !== true);
  if (!asked.length) return s;
  const own = words(line);
  // The last word is still being typed unless a space follows it.
  const typing = !/\s$/.test(query);
  let shared = 0;
  asked.forEach((w, i) => {
    let best = 0;
    for (const o of own) {
      if (o === w) {
        best = 1;
        break;
      }
      if (stem(o) === stem(w)) best = Math.max(best, 0.9);
      else if (typing && i === asked.length - 1 && o.startsWith(w)) best = Math.max(best, 0.8);
      else if (w.length >= 4 && o.length >= 4 && o.slice(0, 4) === w.slice(0, 4)) best = Math.max(best, 0.5);
    }
    shared += best;
  });
  return s + shared / asked.length;
}

/** `items` whose text (`textOf`) answers `query`, best first (ties in page order); every item for no words yet. */
export function match<T>(query: string, items: T[], textOf: (item: T) => string): T[] {
  if (!query.trim()) return items;
  return items
    .map((item, i) => ({ item, i, s: score(query, textOf(item)) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s || a.i - b.i)
    .slice(0, MOST)
    .map((x) => x.item);
}
