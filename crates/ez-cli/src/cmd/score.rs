//! `ez score` and `ez rank`: read a posting, extract its requirements (cached), score masters against it.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, bail};
use resume_core::{
    Bank, Master, PageFacts, Posting, Requirements, Resume, Score, rank, resolve, score,
};
use resume_llm::extract::DEFAULT_EXTRACT_MODEL;
use resume_render::{FontOptions, Renderer};
use serde::Serialize;

use super::job::{self, Source, heading, runtime, today};

#[derive(clap::Args)]
pub struct ScoreArgs {
    #[command(flatten)]
    source: Source,
    /// Model for Extract.
    #[arg(long, default_value = DEFAULT_EXTRACT_MODEL)]
    model: String,
    #[arg(long)]
    master: PathBuf,
    /// Also write posting, requirements and score as JSON here.
    #[arg(long)]
    json: Option<PathBuf>,
}

#[derive(clap::Args)]
pub struct RankArgs {
    #[command(flatten)]
    source: Source,
    /// Model for Extract.
    #[arg(long, default_value = DEFAULT_EXTRACT_MODEL)]
    model: String,
    /// Folder of master JSON files.
    #[arg(long)]
    masters: PathBuf,
}

pub fn score_cmd(args: ScoreArgs) -> anyhow::Result<()> {
    let (posting, req) = runtime()?.block_on(job::requirements(&args.source, &args.model))?;
    let bank: Bank = super::read_json(&args.source.bank)?;
    let master: Master = super::read_json(&args.master)?;
    let (resume, facts) = page(&bank, &master)?;
    let s = score(&resume, &req, &facts, today());
    print_score(&posting, &req, &master, &facts, &s);
    if let Some(path) = &args.json {
        #[derive(Serialize)]
        struct Out<'a> {
            posting: &'a Posting,
            requirements: &'a Requirements,
            master: &'a str,
            facts: &'a PageFacts,
            score: &'a Score,
        }
        super::write_json(
            path,
            &Out {
                posting: &posting,
                requirements: &req,
                master: &master.id,
                facts: &facts,
                score: &s,
            },
        )?;
    }
    Ok(())
}

pub fn rank_cmd(args: RankArgs) -> anyhow::Result<()> {
    let (posting, req) = runtime()?.block_on(job::requirements(&args.source, &args.model))?;
    let bank: Bank = super::read_json(&args.source.bank)?;
    let mut candidates = Vec::new();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&args.masters)
        .with_context(|| format!("reading {}", args.masters.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        bail!("no master JSON files in {}", args.masters.display());
    }
    let start = Instant::now();
    for path in &paths {
        let master: Master = super::read_json(path)?;
        let (resume, facts) = page(&bank, &master)?;
        candidates.push((format!("{} ({})", master.id, master.title), resume, facts));
    }
    let ranked = rank(&candidates, &req, today());
    println!("{}", heading(&posting, &req));
    for (i, (id, total)) in ranked.iter().enumerate() {
        println!("{:>2}. {total:>5.1}  {id}", i + 1);
    }
    println!(
        "{} master(s) resolved, rendered and scored in {:.0} ms",
        ranked.len(),
        start.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}

/// The master resolved and rendered with the bundled fonts: its page facts for the score.
fn page(bank: &Bank, master: &Master) -> anyhow::Result<(Resume, PageFacts)> {
    let resume =
        resolve(bank, master).with_context(|| format!("resolving master {}", master.id))?;
    let template = job::template(master)?;
    let rendered = Renderer::new(&template, FontOptions::default())
        .render(&serde_json::to_string(&resume)?, false)?;
    let facts = PageFacts {
        pages: rendered.metrics.pages,
        page_limit: master.page_limit,
        ats_safe: template.manifest.ats_safe,
        pdf_text: None,
    };
    Ok((resume, facts))
}

fn print_score(
    posting: &Posting,
    req: &Requirements,
    master: &Master,
    facts: &PageFacts,
    s: &Score,
) {
    println!("{}", heading(posting, req));
    println!(
        "master {} · {} page(s) of {} · seniority {:?}\n",
        master.id, facts.pages, facts.page_limit, req.seniority
    );
    let near = if s.total_near > s.total {
        format!("  ({:.1} with near forms)", s.total_near)
    } else {
        String::new()
    };
    println!("Now {:.1}%{near}\n", s.total);
    for c in &s.components {
        let value = if c.present {
            format!("{:.2}", c.value)
        } else {
            "  –".to_owned()
        };
        println!("  {:<12} w{:<3} {value}  {}", c.id, c.weight, c.detail);
    }
    println!("\n  skills");
    for m in &s.skills {
        let found = match (&m.text, &m.section) {
            (Some(t), Some(sec)) => format!("\"{t}\" in {sec}"),
            (Some(t), None) => format!("\"{t}\""),
            _ => String::new(),
        };
        let group = m
            .group
            .as_deref()
            .map(|g| format!(" [{g}]"))
            .unwrap_or_default();
        println!(
            "    {:<8} p{}{} w{:<4} {}{group}  {found}",
            format!("{:?}", m.status).to_lowercase(),
            m.priority,
            if m.required { "*" } else { " " },
            m.weight,
            m.term
        );
    }
    if !s.knockouts.is_empty() {
        println!("\n  check yourself (not scored)");
        for k in &s.knockouts {
            println!("    {:?}: {}", k.kind, k.text);
        }
    }
}
