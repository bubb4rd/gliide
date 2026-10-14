//! A posting's pay: a range in one currency, per year or per hour, either end open ("up to $120K").
//!
//! [`find`] reads it from the posting's text the way a person skims for it: a money range (or one amount) on a line
//! that talks about pay, or under a short heading that does ("Compensation"). Amounts that can't be a salary (a
//! $500 stipend, $100 million in revenue) are left alone, so nothing is better than a wrong guess.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Salary {
    /// The low end in cents (minor units); `None` for "up to".
    pub min_cents: Option<u32>,
    /// The high end in cents; `None` for "from". Equal to `min_cents` for a single amount.
    pub max_cents: Option<u32>,
    /// ISO 4217, upper case: `USD`.
    pub currency: String,
    pub period: Period,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Period {
    Year,
    Hour,
}

impl Salary {
    /// A salary from its parts; `None` without either end, with an end of 0, with the low end over the high, or
    /// with a currency that isn't three letters.
    pub fn new(
        min_cents: Option<u32>,
        max_cents: Option<u32>,
        currency: &str,
        period: Period,
    ) -> Option<Self> {
        let currency = currency.trim().to_ascii_uppercase();
        if currency.len() != 3 || !currency.bytes().all(|b| b.is_ascii_uppercase()) {
            return None;
        }
        match (min_cents, max_cents) {
            (None, None) | (Some(0), _) | (_, Some(0)) => return None,
            (Some(lo), Some(hi)) if lo > hi => return None,
            _ => {}
        }
        Some(Self {
            min_cents,
            max_cents,
            currency,
            period,
        })
    }
}

/// Words that say a line is about pay (matched at a word's start: `salaried`, `compensation`).
const PAY_WORDS: [&str; 7] = [
    "salar",
    "pay",
    "compensat",
    "wage",
    "hourly",
    "earning",
    "ote",
];
/// A heading above the amounts ("Compensation", "Base pay range") has at most this many words.
const HEADING_WORDS: usize = 5;
/// Dollars a year, and an hour, a salary can plausibly be.
const YEAR_RANGE: std::ops::RangeInclusive<u64> = 10_000..=2_000_000;
const HOUR_RANGE: std::ops::RangeInclusive<u64> = 7..=1_000;
/// How far after the amounts the period and the currency code are looked for.
const WINDOW: usize = 40;

/// The pay a posting's `text` states; `None` when it states none this can read with confidence.
pub fn find(text: &str) -> Option<Salary> {
    let mut heading = false;
    for line in text.lines() {
        let words = words(line);
        if words.is_empty() {
            continue;
        }
        let about_pay = words
            .iter()
            .any(|w| PAY_WORDS.iter().any(|p| w.starts_with(p)));
        if (about_pay || heading)
            && let Some(salary) = in_line(line)
        {
            return Some(salary);
        }
        // A heading may also just say "range" ("Hiring Range").
        heading = (about_pay || words.iter().any(|w| w == "range")) && words.len() <= HEADING_WORDS;
    }
    None
}

fn words(line: &str) -> Vec<String> {
    line.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The first salary on `line`.
fn in_line(line: &str) -> Option<Salary> {
    let mut from = 0;
    while let Some(at) = next_amount(line, from) {
        if let Some(salary) = range_at(line, at) {
            return Some(salary);
        }
        from = at.end;
    }
    None
}

/// An amount in `line`: its currency symbol's start, the number's value in cents, and where it ends.
#[derive(Clone, Copy)]
struct Amount {
    start: usize,
    end: usize,
    cents: u64,
    currency: Option<&'static str>,
    /// A period written right after it (`/yr`, `/hour`).
    period: Option<Unit>,
}

#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Year,
    Month,
    Hour,
}

/// The next amount with a currency symbol at or after `from`.
fn next_amount(line: &str, from: usize) -> Option<Amount> {
    let mut i = from;
    while i < line.len() {
        let rest = &line[i..];
        let c = rest.chars().next()?;
        if let Some(currency) = symbol(line, i, c)
            && let Some(amount) = number(line, i + c.len_utf8(), i, Some(currency))
        {
            return Some(amount);
        }
        i += c.len_utf8();
    }
    None
}

/// The currency `c` at `at` stands for: `$` is USD unless written `CA$` or `AU$`.
fn symbol(line: &str, at: usize, c: char) -> Option<&'static str> {
    match c {
        '$' => Some(match &line[..at] {
            p if p.ends_with("CA") || p.ends_with("C") => "CAD",
            p if p.ends_with("AU") || p.ends_with("A") => "AUD",
            _ => "USD",
        }),
        '£' => Some("GBP"),
        '€' => Some("EUR"),
        _ => None,
    }
}

/// The number at `at` (after an optional space), as an amount starting at `start`.
fn number(line: &str, at: usize, start: usize, currency: Option<&'static str>) -> Option<Amount> {
    let bytes = line.as_bytes();
    let mut i = at;
    if bytes.get(i) == Some(&b' ') {
        i += 1;
    }
    let digits_at = i;
    let mut whole: u64 = 0;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'0'..=b'9' => whole = whole.checked_mul(10)? + u64::from(b - b'0'),
            // A thousands separator only between digits.
            b',' if bytes.get(i + 1).is_some_and(u8::is_ascii_digit) && i > digits_at => {}
            _ => break,
        }
        i += 1;
    }
    if i == digits_at {
        return None;
    }
    let mut cents = whole.checked_mul(100)?;
    if bytes.get(i) == Some(&b'.') && bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
        let tens = u64::from(bytes[i + 1] - b'0');
        let ones = bytes
            .get(i + 2)
            .filter(|b| b.is_ascii_digit())
            .map_or(0, |b| u64::from(b - b'0'));
        cents += tens * 10 + ones;
        i += 2;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    }
    let letter_at = |j: usize| bytes.get(j).is_some_and(u8::is_ascii_alphabetic);
    match bytes.get(i) {
        Some(b'k' | b'K') if !letter_at(i + 1) => {
            cents = cents.checked_mul(1000)?;
            i += 1;
        }
        // Millions and billions are never a salary.
        Some(b'm' | b'M' | b'b' | b'B') if !letter_at(i + 1) => return None,
        _ => {}
    }
    let next_word = line[i..].trim_start().to_lowercase();
    if next_word.starts_with("million") || next_word.starts_with("billion") {
        return None;
    }
    let mut period = None;
    if bytes.get(i) == Some(&b'/') {
        let unit_end = line[i + 1..]
            .find(|c: char| !c.is_ascii_alphabetic())
            .map_or(line.len(), |n| i + 1 + n);
        period = unit(&line[i + 1..unit_end].to_lowercase());
        if period.is_some() {
            i = unit_end;
        }
    }
    Some(Amount {
        start,
        end: i,
        cents,
        currency,
        period,
    })
}

fn unit(word: &str) -> Option<Unit> {
    match word {
        "yr" | "year" | "annum" | "annually" | "annual" => Some(Unit::Year),
        "mo" | "month" => Some(Unit::Month),
        "hr" | "hour" | "h" => Some(Unit::Hour),
        _ => None,
    }
}

/// A range starting with `first` (or `first` alone), as a salary.
fn range_at(line: &str, first: Amount) -> Option<Salary> {
    let after = &line[first.end..];
    let gap = after.len() - after.trim_start().len();
    let rest = after.trim_start().to_lowercase();
    let connector = ["-", "–", "—", "‒", "to ", "and "]
        .into_iter()
        .find(|c| rest.starts_with(c));
    let second = connector.and_then(|c| {
        let at = first.end + gap + c.len();
        let at = at + (line[at..].len() - line[at..].trim_start().len());
        // With its own symbol (`CA$` counts: the symbol's prefix is at most two letters), or the first's.
        next_amount(line, at)
            .filter(|a| a.start <= at + 2)
            .or_else(|| number(line, at, at, first.currency))
    });

    let end = second.map_or(first.end, |s| s.end);
    let window = line[end..]
        .chars()
        .take(WINDOW)
        .collect::<String>()
        .to_lowercase();
    let before = line[..first.start].to_lowercase();
    let unit = first
        .period
        .or(second.and_then(|s| s.period))
        .or_else(|| written_unit(&window))
        .or_else(|| written_unit(&before));
    let currency = code(&window).or(first.currency).unwrap_or("USD");

    let (lo, hi) = match second {
        Some(s) => (Some(first.cents), Some(s.cents)),
        None if before.trim_end().ends_with("up to") => (None, Some(first.cents)),
        None if ["starting at", "from", "at least", "minimum of"]
            .iter()
            .any(|w| before.trim_end().ends_with(w)) =>
        {
            (Some(first.cents), None)
        }
        None => (Some(first.cents), Some(first.cents)),
    };
    finish(lo, hi, currency, unit)
}

/// The pay a source states in its own fields: amounts in whole units (JSON-LD `minValue`, Lever's `min`), its
/// currency code, and its unit in any spelling (`YEAR`, `per-hour-wage`). `None` per day or week, or when the
/// amounts can't be a salary.
pub fn from_source(
    min: Option<f64>,
    max: Option<f64>,
    currency: &str,
    unit: &str,
) -> Option<Salary> {
    let words = words(unit);
    if words
        .iter()
        .any(|w| ["day", "daily", "week", "weekly"].contains(&w.as_str()))
    {
        return None;
    }
    let cents = |v: Option<f64>| -> Option<Option<u64>> {
        match v {
            None => Some(None),
            Some(v) if v.is_finite() && v > 0.0 => Some(Some((v * 100.0).round() as u64)),
            Some(_) => None,
        }
    };
    let (lo, hi) = (cents(min)?, cents(max)?);
    finish(lo, hi, currency, written_unit(unit))
}

/// A salary from ends in cents and the period they're per (by their size when unwritten: a thousand or more is a
/// year), when every end is a salary's size for its period.
fn finish(lo: Option<u64>, hi: Option<u64>, currency: &str, unit: Option<Unit>) -> Option<Salary> {
    let top = hi.or(lo)? / 100;
    let (period, factor) = match unit {
        Some(Unit::Year) => (Period::Year, 1),
        Some(Unit::Month) => (Period::Year, 12),
        Some(Unit::Hour) => (Period::Hour, 1),
        None if top >= 1_000 => (Period::Year, 1),
        None => (Period::Hour, 1),
    };
    let plausible = |cents: u64| {
        let dollars = cents * factor / 100;
        match period {
            Period::Year => YEAR_RANGE.contains(&dollars),
            Period::Hour => HOUR_RANGE.contains(&dollars),
        }
    };
    if !lo.into_iter().chain(hi).all(plausible) {
        return None;
    }
    let cents = |c: Option<u64>| -> Option<Option<u32>> {
        match c {
            None => Some(None),
            Some(c) => u32::try_from(c * factor).ok().map(Some),
        }
    };
    Salary::new(cents(lo)?, cents(hi)?, currency, period)
}

/// The period words in `s` name: "per hour", "an hour", "hourly", "a year", "annually"…
fn written_unit(s: &str) -> Option<Unit> {
    let words = words(s);
    let has = |w: &[&str]| words.iter().any(|x| w.contains(&x.as_str()));
    if has(&["hour", "hourly", "hr"]) {
        Some(Unit::Hour)
    } else if has(&["year", "yearly", "annually", "annual", "annum", "yr"]) {
        Some(Unit::Year)
    } else if has(&["month", "monthly"]) {
        Some(Unit::Month)
    } else {
        None
    }
}

/// A currency code written after the amounts: "$75,000–$100,000 USD".
fn code(window: &str) -> Option<&'static str> {
    const CODES: [&str; 8] = ["usd", "cad", "aud", "eur", "gbp", "nzd", "chf", "inr"];
    let first = words(window).into_iter().next()?;
    CODES
        .iter()
        .position(|c| *c == first)
        .map(|i| ["USD", "CAD", "AUD", "EUR", "GBP", "NZD", "CHF", "INR"][i])
}

#[cfg(test)]
mod tests {
    use super::{Period, Salary, find};

    fn year(lo: Option<u32>, hi: Option<u32>, currency: &str) -> Option<Salary> {
        Salary::new(
            lo.map(|d| d * 100),
            hi.map(|d| d * 100),
            currency,
            Period::Year,
        )
    }

    #[test]
    fn ranges_as_postings_write_them() {
        for (text, want) in [
            (
                "Our base pay range for this position is $72,000 -$85,000/year.",
                year(Some(72_000), Some(85_000), "USD"),
            ),
            (
                "We offer a salary range of $75,000–$100,000 USD per year, with compensation tailored to you",
                year(Some(75_000), Some(100_000), "USD"),
            ),
            (
                "- Salary: $80,000 to $105,000 per year, based on experience",
                year(Some(80_000), Some(105_000), "USD"),
            ),
            (
                "The projected compensation range for this position is $114,600.00-$190,200.00. There are",
                year(Some(114_600), Some(190_200), "USD"),
            ),
            (
                "Base pay range\n$80,000.00/yr - $105,000.00/yr",
                year(Some(80_000), Some(105_000), "USD"),
            ),
            (
                // Tiers by place under a heading: the first.
                "Hiring Range\n\nUS Employees in San Francisco/Bay Area, New York City - $106,000 - $137,000\nUS Employees in Boston - $98,000 - $126,000",
                year(Some(106_000), Some(137_000), "USD"),
            ),
            (
                "Compensation: $150K – $200K",
                year(Some(150_000), Some(200_000), "USD"),
            ),
            (
                "Salary: CA$90,000–CA$110,000",
                year(Some(90_000), Some(110_000), "CAD"),
            ),
            (
                "Salary: £45,000 - £55,000",
                year(Some(45_000), Some(55_000), "GBP"),
            ),
            ("Pay: up to $120K a year", year(None, Some(120_000), "USD")),
            (
                "Salary starting at $90,000",
                year(Some(90_000), None, "USD"),
            ),
            (
                "Salary: $8,000 per month",
                year(Some(96_000), Some(96_000), "USD"),
            ),
            (
                "Compensation\n- $45 - $55 per hour",
                Salary::new(Some(4_500), Some(5_500), "USD", Period::Hour),
            ),
            (
                "Hourly rate: $28.50/hr",
                Salary::new(Some(2_850), Some(2_850), "USD", Period::Hour),
            ),
        ] {
            assert_eq!(find(text), want, "{text}");
        }
    }

    #[test]
    fn amounts_that_arent_pay_are_left_alone() {
        for text in [
            // No pay word on the line or in a heading above it.
            "Tuition reimbursement for up to $2,000 per calendar year",
            "$100,000 - $120,000",
            // Pay words, but not a salary's size.
            "Compensation includes a $2,000 home-office stipend",
            "Our compensation team supports $100 million+ in annual revenue",
            "We pay a $1M bonus pool",
            // A long line under a pay heading isn't a heading.
            "Salary is competitive and depends on the role and on many other factors\nThe office budget is $50,000 - $60,000",
        ] {
            assert_eq!(find(text), None, "{text}");
        }
    }

    #[test]
    fn a_salary_needs_an_amount_and_ordered_ends() {
        assert_eq!(Salary::new(None, None, "USD", Period::Year), None);
        assert_eq!(Salary::new(Some(9), Some(5), "USD", Period::Year), None);
        assert_eq!(Salary::new(Some(5), None, "dollars", Period::Year), None);
        assert_eq!(
            Salary::new(Some(5), None, " usd", Period::Year).map(|s| s.currency),
            Some("USD".to_owned())
        );
    }
}
