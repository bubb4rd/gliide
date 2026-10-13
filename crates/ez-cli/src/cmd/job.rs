//! What `score`, `rank` and `tailor` share: reading a posting, extracting its requirements (cached), the master's
//! template, and today's month.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use resume_core::posting::text_hash;
use resume_core::{Master, Method, Posting, Requirements};
use resume_llm::ClaudeCli;
use resume_llm::extract;
use resume_render::Template;

/// Where the posting comes from, the bank, and the model-output cache.
#[derive(clap::Args)]
pub struct Source {
    /// A job link, a posting file (`.md` fixtures keep their front matter as hints), `-` for stdin, or the text.
    pub posting: String,
    #[arg(long)]
    pub bank: PathBuf,
    /// Cache folder: requirements keyed by posting text, prompt and model (and suggestions, for `tailor`).
    #[arg(long, default_value = "local/cache")]
    pub cache: PathBuf,
    /// Ignore cached model output and call the model again.
    #[arg(long)]
    pub fresh: bool,
}

pub fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

/// The posting and its requirements, extracted with `model`: from the cache when this text, prompt and model were
/// extracted before.
pub async fn requirements(src: &Source, model: &str) -> anyhow::Result<(Posting, Requirements)> {
    let posting = read_posting(&src.posting).await?;
    let prompt = &text_hash(&format!("{}\n{}", extract::PROMPT, extract::SCHEMA))[..12];
    let cached = src
        .cache
        .join("requirements")
        .join(model)
        .join(format!("{}-{prompt}.json", &posting.hash[..16]));
    if !src.fresh && cached.exists() {
        eprintln!("requirements: cached ({})", cached.display());
        return Ok((posting, super::read_json(&cached)?));
    }
    let llm = ClaudeCli::discover().await?;
    eprintln!("requirements: extracting with claude -p --model {model} …");
    let (req, out) = extract::extract(&llm, &extract::schema(), &posting, model).await?;
    eprintln!(
        "requirements: {:.1} s{}",
        out.latency().as_secs_f64(),
        if out.repaired() {
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

/// The master's built-in template.
pub fn template(master: &Master) -> anyhow::Result<Template> {
    match master.template.as_str() {
        "classic-serif" => Ok(Template::classic_serif()),
        other => bail!("no built-in template `{other}`"),
    }
}

/// `Role · Company · Location`, then the posting's link.
pub fn heading(posting: &Posting, req: &Requirements) -> String {
    let mut h = format!("{} · {}", req.role, req.company);
    if !req.location.is_empty() {
        h += &format!(" · {}", req.location);
    }
    if let Some(url) = &posting.url {
        h += &format!("\n{url}");
    }
    h
}

/// The current `(year, month)` in UTC, the month a `present` end date stands for.
pub fn today() -> (u32, u32) {
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
