//! `ez tailor`: a posting and a master in; Suggest's changes, fitted on the page, out: the tailored resume, its PDF and
//! highlighted pages, every change with its state and points, and the scores now → projected (ceiling).

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Context;
use resume_core::candidates::bank_candidates;
use resume_core::change::{Change, Draft, Mode, Op, State};
use resume_core::changes::{changes_of, draft_changes};
use resume_core::posting::text_hash;
use resume_core::suggestions::Suggestions;
use resume_core::{Bank, Master, Posting, Requirements, resolve};
use resume_llm::ClaudeCli;
use resume_llm::extract::DEFAULT_EXTRACT_MODEL;
use resume_llm::suggest::{self, DEFAULT_SUGGEST_MODEL, SuggestInput};
use resume_render::{FontOptions, Renderer};
use resume_tailor::{Tailored, tailor};
use serde::Serialize;

use super::job::{self, Source, heading, runtime, today};

/// Bank entries outside the master Suggest sees in bank-aware mode (ARCHITECTURE.md §2).
const CANDIDATES: usize = 5;

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    source: Source,
    #[arg(long)]
    master: PathBuf,
    /// What Suggest may draw from: the master alone, or the master plus bank entries outside it.
    #[arg(long, value_enum, default_value = "master-only")]
    mode: ModeArg,
    /// Your decisions, wording and own changes (a `Draft` JSON).
    #[arg(long)]
    draft: Option<PathBuf>,
    /// Model for Suggest.
    #[arg(long, default_value = DEFAULT_SUGGEST_MODEL)]
    model: String,
    /// Model for Extract.
    #[arg(long, default_value = DEFAULT_EXTRACT_MODEL)]
    extract_model: String,
    /// Folder for tailored.json, resume.pdf, page-N.png (highlighted) and changes.json.
    #[arg(long)]
    out: PathBuf,
    /// PNG pixels per point (2 = 144 ppi).
    #[arg(long, default_value_t = 2.0)]
    scale: f32,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ModeArg {
    MasterOnly,
    BankAware,
}

impl From<ModeArg> for Mode {
    fn from(m: ModeArg) -> Self {
        match m {
            ModeArg::MasterOnly => Mode::MasterOnly,
            ModeArg::BankAware => Mode::BankAware,
        }
    }
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let rt = runtime()?;
    let (posting, req) = rt.block_on(job::requirements(&args.source, &args.extract_model))?;
    let bank: Bank = super::read_json(&args.source.bank)?;
    let master: Master = super::read_json(&args.master)?;
    let draft: Draft = match &args.draft {
        Some(path) => super::read_json(path)?,
        None => Draft::default(),
    };
    let mode = Mode::from(args.mode);
    let resume =
        resolve(&bank, &master).with_context(|| format!("resolving master {}", master.id))?;
    let candidates = match mode {
        Mode::MasterOnly => Vec::new(),
        Mode::BankAware => bank_candidates(&bank, &master, &req, CANDIDATES),
    };
    let input = SuggestInput {
        posting: &posting,
        requirements: &req,
        resume: &resume,
        bank: &bank,
        candidates: &candidates,
    };
    let suggestions = rt.block_on(suggestions(&args, &input, mode))?;

    let mut renderer = Renderer::new(&job::template(&master)?, FontOptions::default());
    let start = Instant::now();
    let changes = changes_of(&suggestions, &bank, &master, &req, mode);
    let changes = draft_changes(&changes, &draft);
    let t = tailor(
        &bank,
        &master,
        &req,
        &changes,
        &draft,
        &mut renderer,
        today(),
    )?;
    let took = start.elapsed();

    write_outputs(&args.out, &t, &changes, &mut renderer, args.scale)?;
    print(
        &posting,
        &req,
        &master,
        mode,
        &candidates,
        &changes,
        &t,
        took,
    );
    Ok(())
}

/// Suggest's output: from the cache when this posting, master (as Suggest reads it, with its bank candidates), mode,
/// prompt and model were run before.
async fn suggestions(
    args: &Args,
    input: &SuggestInput<'_>,
    mode: Mode,
) -> anyhow::Result<Suggestions> {
    let prompt = &text_hash(&format!("{}\n{}", suggest::PROMPT, suggest::SCHEMA))[..12];
    let master = &text_hash(&suggest::input(&SuggestInput {
        posting: &Posting::pasted(String::new()),
        ..*input
    }))[..12];
    let mode_name = match mode {
        Mode::MasterOnly => "master-only",
        Mode::BankAware => "bank-aware",
    };
    let cached = args
        .source
        .cache
        .join("suggestions")
        .join(&args.model)
        .join(format!(
            "{}-{master}-{mode_name}-{prompt}.json",
            &input.posting.hash[..16]
        ));
    if !args.source.fresh && cached.exists() {
        eprintln!("suggestions: cached ({})", cached.display());
        return super::read_json(&cached);
    }
    let llm = ClaudeCli::discover().await?;
    eprintln!(
        "suggestions: claude -p --model {} ({mode_name}, {} bank candidate(s)) …",
        args.model,
        input.candidates.len()
    );
    let (s, out) = suggest::suggest(&llm, &suggest::schema(), input, &args.model).await?;
    eprintln!(
        "suggestions: {:.1} s{}",
        out.latency().as_secs_f64(),
        if out.repaired() {
            " (after one repair)"
        } else {
            ""
        }
    );
    super::write_json(&cached, &s)?;
    Ok(s)
}

/// One change as `changes.json` lists it.
#[derive(Serialize)]
struct Row<'a> {
    #[serde(flatten)]
    change: &'a Change,
    #[serde(flatten)]
    state: &'a State,
    points: Option<f64>,
    /// The line it makes, for line edits.
    after: Option<&'a str>,
    /// Why a change kept in runs long.
    warning: Option<&'a str>,
    ceiling_state: &'a State,
}

fn write_outputs(
    out: &Path,
    t: &Tailored,
    changes: &[Change],
    renderer: &mut Renderer,
    scale: f32,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    for entry in std::fs::read_dir(out)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.starts_with("page-") && name.ends_with(".png") {
            std::fs::remove_file(&path)?;
        }
    }
    super::write_json(&out.join("tailored.json"), &t.resume)?;
    super::write(&out.join("resume.pdf"), &t.rendered.pdf()?)?;
    let preview = renderer.render(&serde_json::to_string(&t.resume)?, true)?;
    for (i, png) in preview.pngs(scale)?.iter().enumerate() {
        super::write(&out.join(format!("page-{}.png", i + 1)), png)?;
    }
    let rows: Vec<Row> = changes
        .iter()
        .map(|c| Row {
            change: c,
            state: &t.states[&c.key],
            points: t.points.get(&c.key).copied(),
            after: t.after.get(&c.key).map(String::as_str),
            warning: t.warnings.get(&c.key).map(String::as_str),
            ceiling_state: &t.ceiling_states[&c.key],
        })
        .collect();
    super::write_json(&out.join("changes.json"), &rows)
}

#[allow(clippy::too_many_arguments)]
fn print(
    posting: &Posting,
    req: &Requirements,
    master: &Master,
    mode: Mode,
    candidates: &[String],
    changes: &[Change],
    t: &Tailored,
    took: std::time::Duration,
) {
    println!("{}", heading(posting, req));
    let mode = match mode {
        Mode::MasterOnly => "master-only".to_owned(),
        Mode::BankAware => format!("bank-aware ({})", candidates.join(", ")),
    };
    println!("master {} · {mode}\n", master.id);
    println!(
        "Now {:.1}% → Projected {:.1}% (ceiling {:.1}%)",
        t.now.total, t.projected.total, t.ceiling.total
    );
    let pages = t.rendered.metrics.pages;
    println!(
        "{pages} page(s) of {} · {}",
        master.page_limit,
        if pages <= master.page_limit {
            "fits"
        } else {
            "OVER THE PAGE LIMIT"
        }
    );
    let count = |f: fn(&State) -> bool| t.states.values().filter(|s| f(s)).count();
    println!(
        "{} change(s): {} in, {} forced, {} off, {} left out; {} flagged by the validator",
        changes.len(),
        count(|s| matches!(s, State::In)),
        count(|s| matches!(s, State::Forced { .. })),
        count(|s| matches!(s, State::Off)),
        count(|s| matches!(s, State::LeftOut { .. })),
        changes.iter().filter(|c| !c.flags.is_empty()).count(),
    );
    println!(
        "tailor step {:.0} ms (changes, fit, scores, points: {} measure + {} fit + {} point compiles)\n",
        took.as_secs_f64() * 1000.0,
        t.compiles.measure,
        t.compiles.fit,
        t.compiles.points
    );
    for c in changes {
        let state = &t.states[&c.key];
        let (label, reason) = match state {
            State::In => ("in", t.warnings.get(&c.key).cloned()),
            State::Off => ("off", None),
            State::LeftOut { reason } => ("left out", Some(reason.clone())),
            State::Forced { warning } => ("forced", warning.clone()),
        };
        let points = t
            .points
            .get(&c.key)
            .map_or_else(|| "    –".to_owned(), |p| format!("{p:>+5.1}"));
        println!(
            "  {label:<8} {points} p{} {:<10} {}",
            c.priority,
            c.section,
            summary(c, t.after.get(&c.key).map(String::as_str))
        );
        println!("{:>34}{}", "", reason.unwrap_or_else(|| c.why.clone()));
    }
}

/// `before → after` for a line edit, else what the op does.
fn summary(c: &Change, after: Option<&str>) -> String {
    match &c.op {
        Op::Rewrite { bullet: line, .. }
        | Op::UseVariant { bullet: line, .. }
        | Op::Set { line, .. }
        | Op::Swap { line, .. }
        | Op::NoteAdd { note: line, .. } => {
            format!("{line}: “{}” → “{}”", c.before, after.unwrap_or("?"))
        }
        Op::Cut { bullet } => format!("cut {bullet}: “{}”", c.before),
        Op::Add { entry, text } => format!("add under {entry}: “{text}”"),
        Op::SkillAdd { group, item } => format!("{group}: + {item}"),
        Op::SkillRemove { group, item } => format!("{group}: − {item}"),
        Op::SkillRename { group, from, to } => format!("{group}: {from} → {to}"),
        Op::SkillMove { from, to, item } => format!("{item}: {from} → {to}"),
        Op::Include { entry, bullets } if bullets.is_empty() => format!("include {entry}"),
        Op::Include { entry, bullets } => format!("include {entry} ({})", bullets.join(", ")),
        Op::Exclude { entry } => format!("exclude {entry}"),
        Op::SwapEntry { out, entry, .. } => format!("{out} ⇄ {entry}"),
        Op::Reorder { section, entries } => format!("order {section}: {}", entries.join(", ")),
        Op::Summary { text } => format!("summary: “{text}”"),
    }
}
