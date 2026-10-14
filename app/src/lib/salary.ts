// A posting's salary range as the UI writes it and reads it back from a field. Amounts are cents (`SalaryRange`).

import type { SalaryRange } from "$lib/api";

/** Hours in a working year: an hourly range sorts among yearly ones by this. */
const HOURS_A_YEAR = 2080;

function money(cents: number, currency: string, compact: boolean): string {
  const whole = cents % 100 === 0;
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency,
      notation: compact ? "compact" : "standard",
      minimumFractionDigits: whole || compact ? 0 : 2,
      maximumFractionDigits: whole || compact ? 0 : 2,
    }).format(cents / 100);
  } catch {
    // A code Intl doesn't know: the number, then the code.
    return `${(cents / 100).toLocaleString()} ${currency}`;
  }
}

/**
 * `$80,000–$105,000 a year`, `Up to $45 an hour`; `short`: `$80K–$105K`, `$45–$55/hr` (a table cell, where the
 * year goes without saying).
 */
export function salaryText(s: SalaryRange, short = false): string {
  const year = s.period === "year";
  const at = (c: number) => money(c, s.currency, short && year);
  const { minCents: lo, maxCents: hi } = s;
  const amount =
    lo !== null && hi !== null
      ? lo === hi
        ? at(lo)
        : `${at(lo)}–${at(hi)}`
      : hi !== null
        ? `Up to ${at(hi)}`
        : `From ${at(lo ?? 0)}`;
  if (short) return year ? amount : `${amount}/hr`;
  return `${amount} ${year ? "a year" : "an hour"}`;
}

/** The range's top (its low end when open above), per year: what Applications sorts by. */
export function perYear(s: SalaryRange): number {
  const top = s.maxCents ?? s.minCents ?? 0;
  return s.period === "year" ? top : top * HOURS_A_YEAR;
}

/**
 * An amount as typed, in cents: `80000`, `80,000`, `$80k`, `45.50`. `null` when blank, `undefined` when it isn't an
 * amount.
 */
export function parseAmount(typed: string): number | null | undefined {
  const t = typed.trim().replace(/^[^\d.]+/, "").replace(/,/g, "").toLowerCase();
  if (!t) return typed.trim() ? undefined : null;
  const m = /^(\d+(?:\.\d{1,2})?)(k?)$/.exec(t);
  if (!m) return undefined;
  const cents = Math.round(Number(m[1]) * 100 * (m[2] ? 1000 : 1));
  return cents > 0 && cents <= 0xffff_ffff ? cents : undefined;
}

/** Cents as the field shows them: `80000`, `45.50`. */
export function amountField(cents: number | null): string {
  if (cents === null) return "";
  return cents % 100 === 0 ? String(cents / 100) : (cents / 100).toFixed(2);
}
