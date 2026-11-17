//! `report.md` and `routes.json` from the recorded runs, and the blind review sheets.
//!
//! Each role builds [`Section`]s: per route, a map of numbers (what `routes.json` holds), shown as a table of metrics
//! × routes with the pass bars of §3.2–3.3 marked, then each posting's median and spread per route.
//!
//! Blind sheets (`<role>/blind.md`) list each posting's outputs under labels shuffled with a fixed seed, so you can't
//! tell the routes apart; `<role>/key.json` maps the labels back. Mark `[x]` on what you'd keep (Tailor) or prefer
//! (Write, Coach); the report joins the marks with the key into a rate per route. A sheet with marks is never
//! rewritten.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use resume_llm::pricing::PRICES_AS_OF;

use super::{Fixtures, Outcome, Run, mean, percentile, recorded};

/// A metric's pass bar.
#[derive(Clone, Copy, Debug)]
pub enum Bar {
    AtLeast(f64),
    AtMost(f64),
}

impl Bar {
    fn passes(self, x: f64) -> bool {
        match self {
            Self::AtLeast(b) => x >= b,
            Self::AtMost(b) => x <= b,
        }
    }

    fn label(self, digits: usize) -> String {
        match self {
            Self::AtLeast(b) => format!("≥ {b:.digits$}"),
            Self::AtMost(b) => format!("≤ {b:.digits$}"),
        }
    }
}

/// One row of a section's table.
pub struct Metric {
    /// Its key in `routes.json`.
    pub key: &'static str,
    pub name: &'static str,
    pub digits: usize,
    pub bar: Option<Bar>,
}

pub const fn m(key: &'static str, name: &'static str, digits: usize, bar: Option<Bar>) -> Metric {
    Metric {
        key,
        name,
        digits,
        bar,
    }
}

/// A route's numbers, by metric key.
pub type Numbers = BTreeMap<&'static str, f64>;

/// A per-posting table: posting → route → the values of its runs.
pub struct PerPosting {
    pub title: &'static str,
    pub digits: usize,
    pub values: BTreeMap<String, BTreeMap<String, Vec<f64>>>,
}

impl PerPosting {
    pub fn new(title: &'static str, digits: usize) -> Self {
        Self {
            title,
            digits,
            values: BTreeMap::new(),
        }
    }

    pub fn push(&mut self, posting: &str, route: &str, x: f64) {
        self.values
            .entry(posting.to_owned())
            .or_default()
            .entry(route.to_owned())
            .or_default()
            .push(x);
    }
}

/// One role's part of the report.
pub struct Section {
    /// Its key in `routes.json`.
    pub key: &'static str,
    pub title: &'static str,
    /// What the numbers mean, under the title.
    pub notes: String,
    pub metrics: Vec<Metric>,
    pub routes: BTreeMap<String, Numbers>,
    pub per_posting: Vec<PerPosting>,
    /// Markdown after the tables.
    pub extra: String,
}

impl Section {
    pub fn new(key: &'static str, title: &'static str, notes: String) -> Self {
        Self {
            key,
            title,
            notes,
            metrics: Vec::new(),
            routes: BTreeMap::new(),
            per_posting: Vec::new(),
            extra: String::new(),
        }
    }

    fn render(&self, md: &mut String) {
        let routes: Vec<&String> = self.routes.keys().collect();
        let _ = writeln!(md, "\n## {}\n\n{}", self.title, self.notes);
        let _ = write!(md, "| Metric | Bar |");
        for r in &routes {
            let _ = write!(md, " `{r}` |");
        }
        let _ = write!(md, "\n|---|---|");
        md.push_str(&"---|".repeat(routes.len()));
        md.push('\n');
        for metric in &self.metrics {
            let _ = write!(
                md,
                "| {} | {} |",
                metric.name.replace('|', "\\|"),
                metric
                    .bar
                    .map_or("–".to_owned(), |b| b.label(metric.digits))
            );
            for r in &routes {
                let cell = match self.routes[*r].get(metric.key) {
                    None => "–".to_owned(),
                    Some(&x) => {
                        let v = format!("{x:.*}", metric.digits);
                        match metric.bar {
                            Some(b) if b.passes(x) => format!("{v} pass"),
                            Some(_) => format!("{v} **fail**"),
                            None => v,
                        }
                    }
                };
                let _ = write!(md, " {cell} |");
            }
            md.push('\n');
        }
        for pp in &self.per_posting {
            if pp.values.is_empty() {
                continue;
            }
            let _ = writeln!(
                md,
                "\n### Per posting: {} (median, min–max, runs)\n",
                pp.title
            );
            let _ = write!(md, "| Posting |");
            for r in &routes {
                let _ = write!(md, " `{r}` |");
            }
            let _ = write!(md, "\n|---|");
            md.push_str(&"---|".repeat(routes.len()));
            md.push('\n');
            for (posting, by) in &pp.values {
                let _ = write!(md, "| `{posting}` |");
                for r in &routes {
                    let cell = by
                        .get(*r)
                        .filter(|v| !v.is_empty())
                        .map_or("–".to_owned(), |v| spread(v, pp.digits));
                    let _ = write!(md, " {cell} |");
                }
                md.push('\n');
            }
        }
        md.push_str(&self.extra);
    }
}

/// `median (min–max, n)`, or the value alone for one run.
fn spread(v: &[f64], digits: usize) -> String {
    let med = median(v).unwrap_or(f64::NAN);
    if v.len() == 1 {
        return format!("{med:.digits$}");
    }
    let (lo, hi) = v
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
            (lo.min(x), hi.max(x))
        });
    format!("{med:.digits$} ({lo:.digits$}–{hi:.digits$}, {})", v.len())
}

/// The middle value (the mean of the two middle ones for an even count).
pub fn median(v: &[f64]) -> Option<f64> {
    let mut v = v.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(v[n / 2]),
        _ => Some((v[n / 2 - 1] + v[n / 2]) / 2.0),
    }
}

/// Runs grouped by route (`agent:model`), in name order.
pub fn by_route<'a>(runs: impl IntoIterator<Item = &'a Run>) -> BTreeMap<String, Vec<&'a Run>> {
    let mut out: BTreeMap<String, Vec<&Run>> = BTreeMap::new();
    for r in runs {
        out.entry(r.route.to_string()).or_default().push(r);
    }
    out
}

/// The rows every role starts with, with the role's bars when it has them: the first-try share, and p50 and p90
/// latency in seconds.
pub fn common_metrics(first_try: Option<f64>, p50: Option<f64>, p90: Option<f64>) -> Vec<Metric> {
    vec![
        m("runs", "Runs", 0, None),
        m(
            "first_try",
            "Valid first try (share)",
            2,
            first_try.map(Bar::AtLeast),
        ),
        m("repaired", "Valid after repair", 0, None),
        m("failed", "Failed", 0, None),
        m("p50_s", "p50 latency s", 1, p50.map(Bar::AtMost)),
        m("p90_s", "p90 latency s", 1, p90.map(Bar::AtMost)),
        m("tokens_in", "Mean input tokens (incl. cache)", 0, None),
        m("tokens_out", "Mean output tokens", 0, None),
        m("cost_usd", "Mean cost $ (API prices)", 4, None),
    ]
}

/// Outcome counts, latency percentiles, mean tokens and cost of `runs`.
pub fn common(runs: &[&Run]) -> Numbers {
    let count = |o| runs.iter().filter(|r| r.outcome == o).count() as f64;
    let lat: Vec<f64> = runs.iter().map(|r| r.secs()).collect();
    let mut n = Numbers::new();
    n.insert("runs", runs.len() as f64);
    n.insert("valid", count(Outcome::Valid));
    n.insert("repaired", count(Outcome::Repaired));
    n.insert("failed", count(Outcome::Failed));
    if !runs.is_empty() {
        n.insert("first_try", count(Outcome::Valid) / runs.len() as f64);
    }
    let mut put = |k, v: Option<f64>| {
        if let Some(v) = v {
            n.insert(k, v);
        }
    };
    put("p50_s", percentile(&lat, 0.5));
    put("p90_s", percentile(&lat, 0.9));
    let called: Vec<&&Run> = runs.iter().filter(|r| !r.attempts.is_empty()).collect();
    put(
        "tokens_in",
        mean(
            called
                .iter()
                .map(|r| (r.tokens.input + r.tokens.cache_write + r.tokens.cache_read) as f64),
        ),
    );
    put(
        "tokens_out",
        mean(called.iter().map(|r| r.tokens.output as f64)),
    );
    put("cost_usd", mean(runs.iter().filter_map(|r| r.cost_usd)));
    n
}

/// Each failed run's first error line, for the end of a section.
pub fn errors<'a>(runs: impl IntoIterator<Item = &'a Run>) -> String {
    let mut s = String::new();
    for r in runs.into_iter().filter(|r| r.error.is_some()) {
        let e = r.error.as_deref().unwrap_or("");
        let _ = writeln!(
            s,
            "- `{}` `{}` #{}: {}",
            r.route,
            r.posting,
            r.n,
            e.lines().next().unwrap_or("").replace('|', "\\|")
        );
    }
    if s.is_empty() {
        s
    } else {
        format!("\n### Errors\n\n{s}")
    }
}

/// Rebuilds `<out>/report.md` and `<out>/routes.json` from every run recorded under `out`.
pub fn run(fx: &Fixtures, out: &Path) -> anyhow::Result<()> {
    let mut sections: Vec<Section> = Vec::new();
    let read = recorded(out, "read")?;
    if !read.is_empty() {
        sections.extend(super::read::section(fx, &read)?);
    }
    for role in ["tailor", "tailor-master-only"] {
        let runs = recorded(out, role)?;
        if !runs.is_empty() {
            sections.extend(super::tailor::section(fx, &runs, role, &out.join(role))?);
        }
    }
    let write = recorded(out, "write")?;
    if !write.is_empty() {
        sections.extend(super::write::section(&write, &out.join("write"))?);
    }
    let coach = recorded(out, "coach")?;
    if !coach.is_empty() {
        sections.extend(super::coach::section(fx, &coach, &out.join("coach"))?);
    }

    let mut md = format!("# Route eval: `{}`\n\n", out.display());
    let _ = writeln!(
        md,
        "Built from the runs recorded there (`<role>/<agent>-<model>/<posting>/<n>.json`) by `ez eval`. Calls ran one \
         at a time. Latency is wall time per call, both turns when repaired. Cost is what the agent reported, else \
         the tokens at the model's API list price (as of {PRICES_AS_OF}); on a subscription it is a measure, not a \
         bill. A bar marked **fail** misses `local/routing/PLAN.md` §3's pass bar."
    );
    if sections.is_empty() {
        md.push_str("\nNo runs recorded yet.\n");
    }
    for s in &sections {
        s.render(&mut md);
    }
    super::super::write(&out.join("report.md"), md.as_bytes())?;
    let mut routes: BTreeMap<&str, &BTreeMap<String, Numbers>> = BTreeMap::new();
    for s in &sections {
        routes.insert(s.key, &s.routes);
    }
    super::super::write_json(&out.join("routes.json"), &routes)?;
    println!(
        "{}\n{}",
        out.join("report.md").display(),
        out.join("routes.json").display()
    );
    Ok(())
}

// ── Blind sheets ────────────────────────────────────────────────────────────────────────────────────────────────────

/// The shuffle's fixed seed: the same runs always get the same labels.
const SEED: &str = "gliide-eval-1";

/// Posting → label → route, as `key.json` holds it.
pub type Key = BTreeMap<String, BTreeMap<String, String>>;

/// 64-bit FNV-1a of `parts`, each ended by a zero byte.
fn fnv(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain([0]) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// `routes` of one posting in shuffled order, labelled `A`, `B`, ….
fn labels<'a>(
    posting: &str,
    routes: impl Iterator<Item = &'a String>,
) -> Vec<(String, &'a String)> {
    let mut rs: Vec<&String> = routes.collect();
    rs.sort_by_key(|r| fnv(&[SEED, posting, r]));
    rs.into_iter()
        .enumerate()
        .map(|(i, r)| (label(i), r))
        .collect()
}

/// `A`…`Z`, then `AA`, `AB`….
fn label(i: usize) -> String {
    let letter = |n: usize| char::from(b'A' + (n % 26) as u8);
    if i < 26 {
        letter(i).to_string()
    } else {
        format!("{}{}", letter(i / 26 - 1), letter(i))
    }
}

/// Writes `<dir>/blind.md` and `<dir>/key.json` from `items` (posting → route → its part of the sheet, holding one
/// `- [ ] …` line per thing to mark), unless the sheet exists with marks in it: those are your review.
pub fn write_blind(
    dir: &Path,
    title: &str,
    how: &str,
    items: &BTreeMap<String, BTreeMap<String, String>>,
) -> anyhow::Result<()> {
    let path = dir.join("blind.md");
    if let Ok(old) = std::fs::read_to_string(&path)
        && !marks(&old).values().all(|&(marked, _)| marked == 0)
    {
        eprintln!("{}: marked, left as it is", path.display());
        return Ok(());
    }
    let mut md = format!(
        "# Blind review: {title}\n\n{how} Labels are shuffled per posting; `key.json` says which route each is. Mark `[x]`, then run `ez eval report`.\n"
    );
    let mut key = Key::new();
    for (posting, by) in items {
        let _ = writeln!(md, "\n## {posting}");
        for (l, route) in labels(posting, by.keys()) {
            let _ = writeln!(md, "\n### {l}\n\n{}", by[route].trim_end());
            key.entry(posting.clone())
                .or_default()
                .insert(l, route.clone());
        }
    }
    super::super::write(&path, md.as_bytes())?;
    super::super::write_json(&dir.join("key.json"), &key)
}

/// `(posting, label)` → (boxes marked, boxes) in a sheet: `## posting`, `### label`, then `- [ ]` / `- [x]` lines.
pub fn marks(md: &str) -> BTreeMap<(String, String), (usize, usize)> {
    let mut out: BTreeMap<(String, String), (usize, usize)> = BTreeMap::new();
    let (mut posting, mut label) = (String::new(), String::new());
    for line in md.lines() {
        if let Some(p) = line.strip_prefix("## ") {
            posting = p.trim().to_owned();
            label.clear();
        } else if let Some(l) = line.strip_prefix("### ") {
            label = l.trim().to_owned();
        } else if !label.is_empty() {
            let t = line.trim_start();
            let marked = t.starts_with("- [x]") || t.starts_with("- [X]");
            if marked || t.starts_with("- [ ]") {
                let e = out.entry((posting.clone(), label.clone())).or_default();
                e.0 += usize::from(marked);
                e.1 += 1;
            }
        }
    }
    out
}

/// Route → (boxes marked, boxes), through `key`, over the postings you reviewed: those with a box marked under any
/// label. A posting left blank says nothing about its changes, so counting its boxes as rejections would sink every
/// route's rate toward 0 when only part of the sheet is marked.
pub fn join(
    marks: &BTreeMap<(String, String), (usize, usize)>,
    key: &Key,
) -> BTreeMap<String, (usize, usize)> {
    let reviewed: std::collections::BTreeSet<&str> = marks
        .iter()
        .filter(|(_, (marked, _))| *marked > 0)
        .map(|((posting, _), _)| posting.as_str())
        .collect();
    let mut out: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for ((posting, label), (marked, boxes)) in marks {
        if !reviewed.contains(posting.as_str()) {
            continue;
        }
        if let Some(route) = key.get(posting).and_then(|ls| ls.get(label)) {
            let e = out.entry(route.clone()).or_default();
            e.0 += marked;
            e.1 += boxes;
        }
    }
    out
}

/// The share of boxes marked per route in `<dir>/blind.md`, over the postings reviewed; `None` until the sheet has a
/// mark.
pub fn rates(dir: &Path) -> anyhow::Result<Option<BTreeMap<String, f64>>> {
    let Ok(md) = std::fs::read_to_string(dir.join("blind.md")) else {
        return Ok(None);
    };
    let marks = marks(&md);
    if marks.values().all(|&(marked, _)| marked == 0) {
        return Ok(None);
    }
    let key: Key = super::super::read_json(&dir.join("key.json"))
        .with_context(|| format!("{} has marks but no key", dir.display()))?;
    Ok(Some(
        join(&marks, &key)
            .into_iter()
            .filter(|(_, (_, boxes))| *boxes > 0)
            .map(|(r, (marked, boxes))| (r, marked as f64 / boxes as f64))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_stable_and_shuffled_per_posting() {
        let routes: Vec<String> = ["claude:sonnet", "claude:opus", "codex:gpt-6-luna"]
            .map(str::to_owned)
            .to_vec();
        let a = labels("northwind", routes.iter());
        assert_eq!(a, labels("northwind", routes.iter().rev()));
        assert_eq!(
            a.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(),
            ["A", "B", "C"]
        );
        // Some posting orders them differently, so position doesn't give a route away.
        assert!(
            ["acme", "globex", "hooli", "initech", "wayne"]
                .iter()
                .any(|p| labels(p, routes.iter())
                    .iter()
                    .map(|(_, r)| *r)
                    .ne(a.iter().map(|(_, r)| *r)))
        );
    }

    #[test]
    fn marks_join_to_the_routes_behind_the_labels() {
        let md = "# Blind review\n\n## p1\n\n### A\n\n- [x] keep — one\n- [ ] keep — two\n\n### B\n\n- [ ] keep — \
                  three\n\n## p2\n\n### A\n\n- [X] keep — four\n\n### B\n\n- [x] keep — five\n- [x] keep — six\n\n\
                  ## p3\n\n### A\n\n- [ ] keep — seven\n\n### B\n\n- [ ] keep — eight\n";
        let key: Key = serde_json::from_str(
            r#"{"p1": {"A": "claude:sonnet", "B": "codex:gpt-6-luna"},
                "p2": {"A": "codex:gpt-6-luna", "B": "claude:sonnet"},
                "p3": {"A": "claude:sonnet", "B": "codex:gpt-6-luna"}}"#,
        )
        .unwrap();
        // p3 has no mark: not reviewed, so its boxes aren't counted as rejections.
        let joined = join(&marks(md), &key);
        assert_eq!(joined["claude:sonnet"], (3, 4));
        assert_eq!(joined["codex:gpt-6-luna"], (1, 2));
    }

    #[test]
    fn a_marked_sheet_is_kept() {
        let dir = std::env::temp_dir().join(format!("ez-eval-blind-{}", std::process::id()));
        let items: BTreeMap<String, BTreeMap<String, String>> = [(
            "p1".to_owned(),
            [
                ("claude:sonnet".to_owned(), "- [ ] best".to_owned()),
                ("codex:gpt-6-luna".to_owned(), "- [ ] best".to_owned()),
            ]
            .into(),
        )]
        .into();
        write_blind(&dir, "t", "Pick one.", &items).unwrap();
        assert_eq!(rates(&dir).unwrap(), None);
        let md = std::fs::read_to_string(dir.join("blind.md")).unwrap();
        let marked = md.replacen("- [ ] best", "- [x] best", 1);
        std::fs::write(dir.join("blind.md"), &marked).unwrap();
        write_blind(&dir, "t", "Pick one.", &items).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("blind.md")).unwrap(),
            marked
        );
        let r = rates(&dir).unwrap().unwrap();
        assert_eq!(r.values().sum::<f64>(), 1.0);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
