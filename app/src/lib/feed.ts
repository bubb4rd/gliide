// The job feed's words and its one shared action: opening a job from it. Overview's New for you and /feed both use
// these, so a row reads and opens the same in both.

import { goto } from "$app/navigation";
import { appError, openFeedJob, type FeedItem, type FeedStatus } from "$lib/api";
import { app } from "$lib/stores/app.svelte";
import { toast, type Source } from "$lib/ui";

/** freehire's filter values with their words, in its order. */
export const SENIORITY = [
  { value: "intern", label: "Intern" },
  { value: "junior", label: "Junior" },
  { value: "middle", label: "Mid-level" },
  { value: "senior", label: "Senior" },
  { value: "lead", label: "Lead" },
  { value: "staff", label: "Staff" },
  { value: "principal", label: "Principal" },
  { value: "c_level", label: "C-level" },
];
export const EMPLOYMENT = [
  { value: "full_time", label: "Full-time" },
  { value: "part_time", label: "Part-time" },
  { value: "contract", label: "Contract" },
  { value: "internship", label: "Internship" },
  { value: "fellowship", label: "Fellowship" },
];
export const CATEGORY = [
  { value: "software_engineering", label: "Software engineering" },
  { value: "backend", label: "Backend" },
  { value: "frontend", label: "Frontend" },
  { value: "fullstack", label: "Full stack" },
  { value: "mobile", label: "Mobile" },
  { value: "devops", label: "DevOps" },
  { value: "sre", label: "SRE" },
  { value: "data_engineering", label: "Data engineering" },
  { value: "data_science", label: "Data science" },
  { value: "data_analytics", label: "Data analytics" },
  { value: "ml_ai", label: "ML and AI" },
  { value: "ai_engineering", label: "AI engineering" },
  { value: "qa", label: "QA" },
  { value: "security", label: "Security" },
  { value: "design", label: "Design" },
  { value: "creative", label: "Creative" },
  { value: "product", label: "Product" },
  { value: "support", label: "Support" },
  { value: "network_engineering", label: "Network engineering" },
];
/** The countries Custom offers first (lowercase ISO, as freehire takes them); any other already chosen is added. */
export const COUNTRIES = ["us", "ca", "gb", "ie", "de", "nl", "fr", "es", "pl", "pt", "in", "au", "nz"];

const regions = new Intl.DisplayNames(undefined, { type: "region" });
/** `us` → "United States". */
export const countryName = (code: string) => regions.of(code.toUpperCase()) ?? code.toUpperCase();

/** freehire and the best-known boards it reads (268 sources, 2026-10-13); the icons are in static/sources/. */
const FREEHIRE_SOURCES: Source[] = [
  { name: "freehire", src: "/sources/freehire.png" },
  { name: "Greenhouse", src: "/sources/greenhouse.png" },
  { name: "Lever", src: "/sources/lever.png" },
  { name: "Ashby", src: "/sources/ashby.png" },
  { name: "Workday", src: "/sources/workday.png" },
  { name: "SmartRecruiters", src: "/sources/smartrecruiters.png" },
];

/** What Search asks: LinkedIn when the feed pulls it (Settings → Feed), then freehire and the boards it reads. */
export const searchSources = (linkedin: boolean): Source[] => [
  ...(linkedin ? [{ name: "LinkedIn", src: "/sources/linkedin.png" }] : []),
  ...FREEHIRE_SOURCES,
];
/** The rest of freehire's boards: 268 less the 5 named above. */
export const SEARCH_SOURCES_NOTE = "and 260+ more boards through freehire";

const DAY_MS = 86_400_000;
const relative = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

/** A day without a time: bare (`2026-10-14`), or at exactly midnight UTC (`2026-10-14T00:00:00Z`), which is how
 * date-only sources (LinkedIn's cards, Workday's "Posted Today", bare dates from the boards) are stored. */
const DATE_ONLY = /^(\d{4})-(\d{2})-(\d{2})(?:T00:00(?::00(?:\.0+)?)?(?:Z|[+-]00:?00))?$/;

/** When `iso` was, in ms. A date without a time is that calendar day where you are (its local midnight), not
 * midnight UTC, which west of UTC falls on the evening before. NaN when it doesn't parse. */
function when(iso: string): number {
  const day = DATE_ONLY.exec(iso.trim());
  return day ? new Date(+day[1], +day[2] - 1, +day[3]).getTime() : Date.parse(iso);
}

/** How long ago `iso` was, in whole local days ("today", "yesterday", "3 days ago"); null when it doesn't parse. */
function daysAgo(iso: string): string | null {
  const at = when(iso);
  if (Number.isNaN(at)) return null;
  const start = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((start(new Date()) - start(new Date(at))) / DAY_MS);
  return relative.format(-Math.max(0, days), "day");
}

/** The employer's posting age ("2 days ago"), else when its source found it ("Found today"). */
export function postedText(item: FeedItem): string {
  const posted = item.postedAt ? daysAgo(item.postedAt) : null;
  if (posted) return posted.charAt(0).toUpperCase() + posted.slice(1);
  const found = daysAgo(item.foundAt);
  return found ? `Found ${found}` : "";
}

/** When the job went up, as `postedText` dates it (the employer's date, else when its source found it), in ms; 0 when
 * neither parses. */
export function postedTime(item: FeedItem): number {
  const posted = item.postedAt ? when(item.postedAt) : NaN;
  if (!Number.isNaN(posted)) return posted;
  const found = when(item.foundAt);
  return Number.isNaN(found) ? 0 : found;
}

/** What the feed is doing now, as one sentence; null when it's idle. */
export function activity(status: FeedStatus, items: FeedItem[]): string | null {
  if (status.checking) {
    const company = items.find((i) => i.slug === status.checking)?.company;
    return company ? `Checking ${company}…` : "Checking a posting…";
  }
  if (status.pulling) return "Pulling new postings…";
  return null;
}

/** "12 of 20 checked today". */
export const checkedToday = (status: FeedStatus) => `${status.checkedToday} of ${status.dailyChecks} checked today`;

/** Reads the job's posting if it hasn't been (it joins Recent and Applications), then opens its review. */
export async function openItem(item: FeedItem): Promise<void> {
  try {
    const row = await openFeedJob(item.slug);
    void app.refreshJobs();
    await goto(`/jobs/${row.id}`);
  } catch (e) {
    toast(`Couldn’t open the ${item.company || item.role} posting: ${appError(e).message}`, { tone: "danger" });
  }
}
