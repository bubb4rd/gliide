//! `ez score` and `ez rank`: read a posting, extract its requirements (cached), score masters against it.

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use resume_core::posting::text_hash;
use resume_core::{
    Bank, Master, Method, PageFacts, Posting, Requirements, Resume, Score, rank, resolve, score,
};
use resume_llm::ClaudeCli;
use resume_llm::extract::{self, DEFAULT_EXTRACT_MODEL};
use resume_render::{FontOptions, Renderer, Template};
use serde::Serialize;

/// Where the posting comes from and how its requirements are extracted; shared by `score` and `rank`.
#[derive(clap::Args)]
pub struct Source {
    /// A job link, a posting file (`.md` fixtures keep their front matter as hints), `-` for stdin, or the text.
    posting: String,
    #[arg(long)]
    bank: PathBuf,
    /// Model for Extract.
    #[arg(long, default_value = DEFAULT_EXTRACT_MODEL)]
    model: String,
    /// Extract cache folder: requirements keyed by posting text, prompt and model.
    #[arg(long, default_value = "local/cache")]
    cache: PathBuf,
    /// Ignore a cached extraction and run Extract again.
    #[arg(long)]
    fresh: bool,
}

#[derive(clap::Args)]
pub struct ScoreArgs {
    #[command(flatten)]
    source: Source,
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
    /// Folder of master JSON files.
    #[arg(long)]
    masters: PathBuf,
}

pub fn score_cmd(args: ScoreArgs) -> anyhow::Result<()> {
    let (posting, req) = runtime()?.block_on(requirements(&args.source))?;
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
    let (posting, req) = runtime()?.block_on(requirements(&args.source))?;
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

fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

/// The posting and its requirements: from the cache when this text, prompt and model were extracted before.
async fn requirements(src: &Source) -> anyhow::Result<(Posting, Requirements)> {
    let posting = read_posting(&src.posting).await?;
    let prompt = &text_hash(&format!("{}\n{}", extract::PROMPT, extract::SCHEMA))[..12];
    let cached = src
        .cache
        .join("requirements")
        .join(&src.model)
        .join(format!("{}-{prompt}.json", &posting.hash[..16]));
    if !src.fresh && cached.exists() {
        eprintln!("requirements: cached ({})", cached.display());
        return Ok((posting, super::read_json(&cached)?));
    }
    let llm = ClaudeCli::discover().await?;
    eprintln!(
        "requirements: extracting with claude -p --model {} …",
        src.model
    );
    let (req, out) = extract::extract(&llm, &extract::schema(), &posting, &src.model).await?;
    eprintln!(
        "requirements: {:.1} s{}",
        out.latency().as_secs_f64(),
        if out.attempts.len() > 1 {
            " (after one repair)"
        } else {
            ""
        }
    );
    super::write_json(&cached, &req)?;
    Ok((posting, req))
}

async fn read_posting(input: &str) -> anyhow::Result<Posting> {
    if input == "-" {
        let text = std::io::read_to_string(std::io::stdin())?;
        return Ok(resume_ingest::read(&text).await?);
    }
    let path = Path::new(input);
    if path.is_file() {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        return Ok(fixture_posting(&text));
    }
    Ok(resume_ingest::read(input).await?)
}

/// A posting file: optional `---` front matter (`title`, `company`, `location`, `source`, `method`), then the text.
/// Front matter of a pasted fixture (`method: pasted text`) is ignored, as the app would have none.
fn fixture_posting(file: &str) -> Posting {
    let Some((front, body)) = file
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
    else {
        return Posting::pasted(file.trim().to_owned());
    };
    let field = |key: &str| {
        front.lines().find_map(|l| {
            l.strip_prefix(key)
                .and_then(|v| v.strip_prefix(':'))
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
    };
    let text = body.trim().to_owned();
    if field("method").is_none_or(|m| m.starts_with("pasted")) {
        return Posting::pasted(text);
    }
    let source = field("source").filter(|s| s.starts_with("http"));
    Posting::new(
        Method::Html,
        source.clone(),
        source,
        field("title"),
        field("company"),
        field("location"),
        text,
    )
}

/// The master resolved and rendered with the bundled fonts: its page facts for the score.
fn page(bank: &Bank, master: &Master) -> anyhow::Result<(Resume, PageFacts)> {
    let resume =
        resolve(bank, master).with_context(|| format!("resolving master {}", master.id))?;
    let template = match master.template.as_str() {
        "classic-serif" => Template::classic_serif(),
        other => bail!("no built-in template `{other}`"),
    };
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

fn heading(posting: &Posting, req: &Requirements) -> String {
    let mut h = format!("{} · {}", req.role, req.company);
    if !req.location.is_empty() {
        h += &format!(" · {}", req.location);
    }
    if let Some(url) = &posting.url {
        h += &format!("\n{url}");
    }
    h
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

/// The current `(year, month)` in UTC, the month a `present` end date stands for.
fn today() -> (u32, u32) {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year as u32, month as u32)
}
