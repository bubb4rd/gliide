use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use resume_core::{Bank, Master, Resume, resolve};
use resume_render::{FontOptions, Renderer, Template};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    bank: PathBuf,
    #[arg(long)]
    master: PathBuf,
    /// Folder for resume.pdf, page-N.svg, page-N.png and metrics.json.
    #[arg(long)]
    out: PathBuf,
    /// Highlight changed lines and added skills in the SVG and PNG pages (the PDF never is).
    #[arg(long)]
    highlight: bool,
    /// Also load system fonts (and Typst's embedded ones), as the `typst` CLI does.
    #[arg(long)]
    system_fonts: bool,
    /// PNG pixels per point (2 = 144 ppi).
    #[arg(long, default_value_t = 2.0)]
    scale: f32,
    /// Template folder (resume.typ + manifest.json); default: the master's built-in template.
    #[arg(long)]
    template: Option<PathBuf>,
    /// After rendering, time N warm re-renders in this process: unchanged, then with one bullet edited each time.
    #[arg(long, value_name = "N")]
    bench: Option<usize>,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let bank: Bank = super::read_json(&args.bank)?;
    let master: Master = super::read_json(&args.master)?;
    let resume =
        resolve(&bank, &master).with_context(|| format!("resolving {}", args.master.display()))?;
    let template = match &args.template {
        Some(dir) => {
            Template::load(dir).with_context(|| format!("loading template {}", dir.display()))?
        }
        None if master.template == "classic-serif" => Template::classic_serif(),
        None => bail!(
            "no built-in template `{}`; pass --template <dir>",
            master.template
        ),
    };
    let data = serde_json::to_string(&resume)?;

    let start = Instant::now();
    let mut renderer = Renderer::new(
        &template,
        FontOptions {
            system: args.system_fonts,
        },
    );
    let plain = renderer.render(&data, false)?;
    let cold = start.elapsed();
    for w in &plain.warnings {
        eprintln!("{w}");
    }
    let preview = if args.highlight {
        Some(renderer.render(&data, true)?)
    } else {
        None
    };
    let pages = preview.as_ref().unwrap_or(&plain);

    std::fs::create_dir_all(&args.out)?;
    clear_pages(&args.out)?;
    super::write(&args.out.join("resume.pdf"), &plain.pdf()?)?;
    for (i, svg) in pages.svgs().iter().enumerate() {
        super::write(
            &args.out.join(format!("page-{}.svg", i + 1)),
            svg.as_bytes(),
        )?;
    }
    for (i, png) in pages.pngs(args.scale)?.iter().enumerate() {
        super::write(&args.out.join(format!("page-{}.png", i + 1)), png)?;
    }
    super::write_json(&args.out.join("metrics.json"), &plain.metrics)?;
    println!(
        "{}: {} page(s), {} bullets, {} skills lines; first render {}",
        args.out.display(),
        plain.metrics.pages,
        plain.metrics.bullets.len(),
        plain.metrics.groups.len(),
        ms(cold)
    );

    if let Some(n) = args.bench {
        bench(&mut renderer, &resume, n.max(1), args.scale)?;
    }
    Ok(())
}

/// Removes `page-N.svg`/`page-N.png` left by an earlier, longer render.
fn clear_pages(dir: &std::path::Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.starts_with("page-") && (name.ends_with(".svg") || name.ends_with(".png")) {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn bench(renderer: &mut Renderer, resume: &Resume, n: usize, scale: f32) -> anyhow::Result<()> {
    let data = serde_json::to_string(resume)?;
    let same = time(n, |_| renderer.render(&data, false).map(drop))?;
    let mut edited = resume.clone();
    let Some(bullet) = edited
        .sections
        .iter_mut()
        .flat_map(|s| &mut s.entries)
        .flat_map(|e| &mut e.bullets)
        .next()
    else {
        bail!("--bench needs a bullet to edit");
    };
    let text = bullet.text.clone();
    let edit = time(n, |i| {
        let bullet = edited
            .sections
            .iter_mut()
            .flat_map(|s| &mut s.entries)
            .flat_map(|e| &mut e.bullets)
            .next();
        bullet.expect("checked above").text = format!("{text} {i}");
        renderer
            .render(&serde_json::to_string(&edited).expect("serializes"), false)
            .map(drop)
    })?;
    let doc = renderer.render(&data, false)?;
    let pdf = time(n, |_| doc.pdf().map(drop))?;
    let svg = time(n, |_| {
        drop(doc.svgs());
        anyhow::Ok(())
    })?;
    let png = time(n, |_| doc.pngs(scale).map(drop))?;
    println!("warm re-render, {n} runs each (median / min):");
    for (what, t) in [
        ("compile, unchanged", same),
        ("compile, one bullet edited", edit),
        ("PDF export", pdf),
        ("SVG export", svg),
        ("PNG export", png),
    ] {
        println!("  {what:<28} {} / {}", ms(t.0), ms(t.1));
    }
    Ok(())
}

/// Median and minimum of `n` runs of `f`.
fn time<E: Into<anyhow::Error>>(
    n: usize,
    mut f: impl FnMut(usize) -> Result<(), E>,
) -> anyhow::Result<(Duration, Duration)> {
    let mut runs = Vec::with_capacity(n);
    for i in 0..n {
        let start = Instant::now();
        f(i).map_err(Into::into)?;
        runs.push(start.elapsed());
    }
    runs.sort();
    Ok((runs[n / 2], runs[0]))
}

fn ms(d: Duration) -> String {
    format!("{:.1} ms", d.as_secs_f64() * 1000.0)
}
