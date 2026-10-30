// Model ids as people say them, when no agent lists a name for one: `claude-sonnet-5-5-20261001` → "Sonnet 5.5",
// `claude-3-5-haiku-20241022` → "Haiku 3.5", the alias `opus` → "Opus", `gpt-6-luna` → "GPT-6 Luna",
// `gpt-5.6-terra` → "GPT-5.6 Terra", `gemini-3.8-flash-medium` → "Gemini 3.8 Flash (Medium)", any other name
// title-cased ("astra" → "Astra").

/** Trailing reasoning efforts, said in parentheses after the name. */
const EFFORT = /^(low|medium|high|xhigh|max)$/i;
/** Words said in capitals. */
const ACRONYM = /^(gpt|oss)$/i;

const isVersion = (part: string) => /^\d+(\.\d+)*$/.test(part);
const word = (part: string) =>
  ACRONYM.test(part)
    ? part.toUpperCase()
    : /^o\d/.test(part)
      ? part
      : part[0].toUpperCase() + part.slice(1);

/**
 * `id` (a model id, alias or name) by name: its words title-cased, its version numbers joined by dots (after the name
 * for Claude's models, in place for others, a GPT's joined to it by a hyphen), a trailing effort in parentheses.
 */
export function modelName(id: string): string {
  const trimmed = id.trim().replace(/\[[^\]]*\]$/, "");
  const claude = /^(claude-|sonnet|opus|haiku)/i.test(trimmed);
  const parts = trimmed
    .replace(/^claude-/i, "")
    .replace(/-\d{8}$/, "")
    .split(/[-_\s]+/)
    .filter(Boolean);
  const effort = parts.length > 1 && EFFORT.test(parts[parts.length - 1]) ? parts.pop() : undefined;

  // Runs of version numbers become one ("5-5" → "5.5").
  const tokens: { version: boolean; text: string }[] = [];
  for (const part of parts) {
    const last = tokens[tokens.length - 1];
    if (isVersion(part) && last?.version) last.text += `.${part}`;
    else tokens.push({ version: isVersion(part), text: part });
  }
  if (!tokens.some((t) => !t.version)) return id.trim();

  let name: string;
  if (claude) {
    const words = tokens.filter((t) => !t.version).map((t) => word(t.text));
    const version = tokens.filter((t) => t.version).map((t) => t.text).join(".");
    name = version ? `${words.join(" ")} ${version}` : words.join(" ");
  } else {
    const out: string[] = [];
    for (const t of tokens) {
      const prev = out.length - 1;
      if (t.version && prev >= 0 && out[prev] === "GPT") out[prev] += `-${t.text}`;
      else out.push(t.version ? t.text : word(t.text));
    }
    name = out.join(" ");
  }
  return effort ? `${name} (${word(effort)})` : name;
}
