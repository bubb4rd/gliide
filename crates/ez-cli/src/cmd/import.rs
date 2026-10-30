use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use resume_core::import::{Choices, master_id, merge, plan};
use resume_core::{Bank, Master, Resume};
use resume_llm::import::{self, Link};
use resume_llm::{Agent, AgentKind};
use resume_pipeline::{ResumeSource, read_resume};

use super::job::{AgentArg, runtime};

#[derive(clap::Args)]
pub struct Args {
    /// The resume: .pdf, .docx, .md, .txt, or a v1 resume JSON (a legacy v1 master.json, or an earlier run's
    /// imported.json, which skips the model call).
    file: PathBuf,
    /// The bank to merge into; without one, the file becomes the bank.
    #[arg(long)]
    bank: Option<PathBuf>,
    /// The new master's title.
    #[arg(long, default_value = "Imported")]
    title: String,
    /// Entry ids of the file (as imported.json has them) to add as entries of their own instead of merging them into
    /// the same bank entry.
    #[arg(long, value_delimiter = ',')]
    separate: Vec<String>,
    /// Add the file's contact items the bank lacks.
    #[arg(long)]
    contact: bool,
    /// The agent CLI the model call runs on.
    #[arg(long, value_enum, default_value = "claude")]
    agent: AgentArg,
    /// Model for Import; the agent's default unless given.
    #[arg(long)]
    model: Option<String>,
    /// Folder for imported.json (the model's copy of the file), bank.json and masters/<id>.json.
    #[arg(long)]
    out: PathBuf,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let (resume, pages) = match read_resume(&args.file)? {
        ResumeSource::AtsFit(r) => (r, None),
        ResumeSource::File(f) => {
            let links: Vec<Link<'_>> = f
                .links
                .iter()
                .map(|l| Link {
                    text: &l.text,
                    url: &l.url,
                })
                .collect();
            let start = Instant::now();
            let agent = AgentKind::from(args.agent);
            let model = args.model.as_deref().unwrap_or(agent.default_model());
            let r: Resume = runtime()?.block_on(async {
                let llm = Agent::discover(agent).await?;
                eprintln!(
                    "import: {} --model {model} ({} chars, {} links) …",
                    agent.bin(),
                    f.text.len(),
                    links.len()
                );
                let (r, _) =
                    import::import(&llm, &import::schema(), &f.text, &links, model).await?;
                anyhow::Ok(r)
            })?;
            eprintln!("import: {:.1} s", start.elapsed().as_secs_f64());
            (r, f.pages)
        }
    };
    super::write_json(&args.out.join("imported.json"), &resume)?;

    let bank: Option<Bank> = args.bank.as_deref().map(super::read_json).transpose()?;
    let p = plan(bank.as_ref(), &resume);
    for w in &p.warnings {
        eprintln!("warning: {w}");
    }
    for (sp, s) in p.sections.iter().zip(&p.incoming.sections) {
        let into = sp.into.as_deref().unwrap_or("new section");
        eprintln!("{} → {into}", s.title);
        for (ep, e) in sp.entries.iter().zip(&s.entries) {
            match &ep.same {
                Some(same) => {
                    let new = same.bullets.iter().filter(|b| b.is_none()).count();
                    let differs: Vec<&str> = same.differs.iter().map(|f| f.as_str()).collect();
                    eprintln!(
                        "  {} [{}] = {}: {} of {} bullets new{}",
                        e.name,
                        e.id,
                        same.entry,
                        new,
                        same.bullets.len(),
                        if differs.is_empty() {
                            String::new()
                        } else {
                            format!("; bank's {} kept", differs.join(", "))
                        }
                    );
                }
                None => eprintln!("  {} [{}]: new, {} bullets", e.name, e.id, e.bullets.len()),
            }
        }
        for (gp, g) in sp.groups.iter().zip(&s.groups) {
            let line = gp.same.as_deref().unwrap_or("new line");
            eprintln!("  {} → {line}: {} new items", g.label, gp.new_items.len());
        }
    }
    for c in &p.contact {
        eprintln!(
            "contact not in the bank{}: {}",
            if args.contact { " (added)" } else { "" },
            c.text
        );
    }

    let choices = Choices {
        separate: args.separate.into_iter().collect(),
        contact: if args.contact {
            (0..p.contact.len()).collect()
        } else {
            BTreeSet::new()
        },
    };
    let merged = merge(bank.as_ref(), &p, &choices);
    let masters = args.out.join("masters");
    let taken: Vec<String> = std::fs::read_dir(&masters)
        .into_iter()
        .flatten()
        .filter_map(|e| Some(e.ok()?.path().file_stem()?.to_str()?.to_owned()))
        .collect();
    let taken: Vec<&str> = taken.iter().map(String::as_str).collect();
    let master = Master {
        id: master_id(&args.title, &taken),
        title: args.title,
        template: "classic-serif".into(),
        page_limit: pages.map_or(1, |n| n.clamp(1, 2) as u32),
        sections: merged.sections,
    };
    resume_core::resolve(&merged.bank, &master)?;
    let bank_path = args.out.join("bank.json");
    let master_path = masters.join(format!("{}.json", master.id));
    super::write_json(&bank_path, &merged.bank)?;
    super::write_json(&master_path, &master)?;
    println!(
        "{}\n{}\n{}",
        args.out.join("imported.json").display(),
        bank_path.display(),
        master_path.display()
    );
    Ok(())
}
