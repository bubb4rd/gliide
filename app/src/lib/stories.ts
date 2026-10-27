// Stories as the screens name them: their themes, how long one takes to tell, and the question Tell it asks.

import type { StoryTheme, StoryView } from "$lib/api";

export const THEMES: { value: StoryTheme; label: string; ask: string }[] = [
  { value: "conflict", label: "Conflict", ask: "Tell me about a time you disagreed with someone you worked with." },
  { value: "failure", label: "Failure", ask: "Tell me about a time something you worked on went wrong." },
  { value: "leadership", label: "Leadership", ask: "Tell me about a time you led others through something." },
  { value: "ambiguity", label: "Ambiguity", ask: "Tell me about a time you had to act without a clear answer." },
  { value: "deadline", label: "Deadline", ask: "Tell me about a time you had to deliver under a tight deadline." },
  { value: "impact", label: "Impact", ask: "Tell me about the work you're proudest of, and what it changed." },
  { value: "learning", label: "Learning", ask: "Tell me about a time you had to learn something new quickly." },
  { value: "teamwork", label: "Teamwork", ask: "Tell me about a time you worked closely with a team to ship something." },
  { value: "initiative", label: "Initiative", ask: "Tell me about something you did that no one asked you to." },
];

export const themeLabel = (theme: StoryTheme) => THEMES.find((t) => t.value === theme)?.label ?? theme;

/** "about 1 min 40 s", "about 40 s": how long a story takes to tell. */
export function tellTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.round((seconds % 60) / 10) * 10;
  if (!m) return `about ${Math.max(s, 10)} s`;
  return s ? `about ${m} min ${s} s` : `about ${m} min`;
}

/** The question Tell it asks of a story: its first theme's, else one about the story itself. */
export function askFor(story: StoryView): string {
  const theme = THEMES.find((t) => story.themes.includes(t.value));
  return theme?.ask ?? `Tell me about the time you ${story.title.charAt(0).toLowerCase()}${story.title.slice(1)}.`;
}
