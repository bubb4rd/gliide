//! The Write role: a cover letter per posting from the resolved `swe` master, untailored (§3.4). Validity, latency
//! and length, and a blind sheet for picking the best letter of each posting.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use resume_bench::{Fixtures, Role, Run, mean, run_one, words};
use resume_llm::cover::Letter;
use serde::Deserialize;

use super::report::{
    PerPosting, Section, by_route, common, common_metrics, errors, m, rates, write_blind,
};
use super::{RunArgs, each};

pub async fn run(fx: &Fixtures, args: &RunArgs, out: &Path) -> anyhow::Result<()> {
    let items: Vec<String> = fx
        .chosen(&args.only)
        .into_iter()
        .map(|f| f.id.clone())
        .collect();
    each(
        "write",
        Role::Write,
        args,
        out,
        &items,
        async |llm, route, id, n| run_one(llm, route, Role::Write, fx, id, n).await,
    )
    .await
}

pub fn section(runs: &[Run], dir: &Path) -> anyhow::Result<Vec<Section>> {
    let mut s = Section::new(
        "write",
        "Write: cover letter",
        "From the posting and the `swe` master resolved, no tailoring. Words: greeting, paragraphs and closing. \
         Preferred: the share of postings whose `[x] best` in `write/blind.md` is this route's, once you've marked it.\n"
            .to_owned(),
    );
    s.metrics = common_metrics(None, None, None);
    s.metrics.extend([
        m("words", "Words, mean", 0, None),
        m("paragraphs", "Paragraphs, mean", 1, None),
        m("preferred", "Blind preference", 2, None),
    ]);
    let pref = rates(dir)?;
    let mut words_pp = PerPosting::new("Words", 0);
    let mut lat_pp = PerPosting::new("Latency s", 1);
    let mut blind: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (route, rs) in by_route(runs) {
        let mut n = common(&rs);
        let letters: Vec<(&Run, Letter)> = rs
            .iter()
            .filter(|r| r.ok())
            .filter_map(|r| Some((*r, Letter::deserialize(r.output.as_ref()?).ok()?)))
            .collect();
        for (k, v) in [
            ("words", mean(letters.iter().map(|(_, l)| words(l) as f64))),
            (
                "paragraphs",
                mean(letters.iter().map(|(_, l)| l.paragraphs.len() as f64)),
            ),
            (
                "preferred",
                pref.as_ref().and_then(|p| p.get(&route).copied()),
            ),
        ] {
            if let Some(v) = v {
                n.insert(k, v);
            }
        }
        for r in &rs {
            lat_pp.push(&r.item, &route, r.secs());
        }
        for (r, l) in &letters {
            words_pp.push(&r.item, &route, words(l) as f64);
            let sheet = blind.entry(r.item.clone()).or_default();
            if !sheet.contains_key(&route) {
                let mut body = String::new();
                for part in std::iter::once(&l.greeting)
                    .chain(&l.paragraphs)
                    .chain(std::iter::once(&l.closing))
                {
                    let _ = writeln!(body, "> {}\n>", part.replace('\n', "\n> "));
                }
                body.push_str("\n- [ ] best\n");
                sheet.insert(route.clone(), body);
            }
        }
        s.routes.insert(route, n);
    }
    s.per_posting = vec![words_pp, lat_pp];
    s.extra = errors(runs);
    write_blind(
        dir,
        s.title,
        "Each route's letter for a posting. Mark `[x] best` on the one letter per posting you'd send.",
        &blind,
    )?;
    Ok(vec![s])
}
