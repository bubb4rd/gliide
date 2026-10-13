//! Reads a posting and prints what was read.
//!
//! ```text
//! cargo run -p resume-ingest --example read -- <url-or-text>
//! cargo run -p resume-ingest --example read -- --record fixtures/ingest/<case> <url>
//! ```
//!
//! `--record DIR` also writes the response into `DIR` as a test case (`resume_ingest::record`); `--full` prints the
//! whole text instead of its first lines.

use std::process::ExitCode;

use resume_ingest::record::Recorder;
use resume_ingest::{HttpFetcher, read_with};

const FIRST_LINES: usize = 12;

#[tokio::main]
async fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let full = args.iter().any(|a| a == "--full");
    args.retain(|a| a != "--full");
    let record = match args.iter().position(|a| a == "--record") {
        Some(i) if i + 1 < args.len() => {
            let dir = args.remove(i + 1);
            args.remove(i);
            Some(dir)
        }
        Some(_) => {
            eprintln!("--record needs a directory");
            return ExitCode::FAILURE;
        }
        None => None,
    };
    let input = args.join(" ");
    if input.trim().is_empty() {
        eprintln!("usage: read [--record DIR] [--full] <url-or-text>");
        return ExitCode::FAILURE;
    }

    let result = match record {
        Some(dir) => {
            let fetched = today();
            read_with(
                &Recorder::new(HttpFetcher::new(), dir, &input, &fetched),
                &input,
            )
            .await
        }
        None => read_with(&HttpFetcher::new(), &input).await,
    };
    let posting = match result {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("       {e:?}");
            return ExitCode::FAILURE;
        }
    };
    let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".to_owned());
    println!(
        "method:   {}",
        serde_json::to_value(posting.method).unwrap_or_default()
    );
    println!("title:    {}", show(&posting.title));
    println!("company:  {}", show(&posting.company));
    println!("location: {}", show(&posting.location));
    println!("url:      {}", show(&posting.url));
    println!(
        "text:     {} chars, {} lines",
        posting.text.chars().count(),
        posting.text.lines().count()
    );
    println!("hash:     {}", posting.hash);
    println!("---");
    let shown = if full { usize::MAX } else { FIRST_LINES };
    for line in posting.text.lines().take(shown) {
        println!("{line}");
    }
    ExitCode::SUCCESS
}

/// `YYYY-MM-DD` (UTC) for the recording's `fetched`.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (era * 400 + yoe + i64::from(m <= 2), m, d)
}
