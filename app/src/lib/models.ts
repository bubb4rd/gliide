// Model ids as people say them: `claude-sonnet-5-5-20261001` → "Sonnet 5.5", `claude-3-5-haiku-20241022` →
// "Haiku 3.5", the alias `opus` → "Opus", any other name title-cased ("astra" → "Astra").

/** `id` (a model id, alias or name) by name: its family words title-cased, then its version numbers joined by dots. */
export function modelName(id: string): string {
  const bare = id
    .trim()
    .replace(/\[[^\]]*\]$/, "")
    .replace(/^claude-/i, "")
    .replace(/-\d{8}$/, "");
  const parts = bare.split(/[-_\s]+/).filter(Boolean);
  const words = parts.filter((p) => !/^\d+$/.test(p));
  const version = parts.filter((p) => /^\d+$/.test(p)).join(".");
  if (!words.length) return id.trim();
  const name = words.map((w) => w[0].toUpperCase() + w.slice(1)).join(" ");
  return version ? `${name} ${version}` : name;
}
