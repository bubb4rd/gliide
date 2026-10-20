use std::path::PathBuf;

use resume_core::{Resume, migrate_v1};

#[derive(clap::Args)]
pub struct Args {
    /// A legacy v1 master.json.
    v1: PathBuf,
    /// Folder for bank.json and masters/swe.json.
    #[arg(long)]
    out: PathBuf,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    let v1: Resume = super::read_json(&args.v1)?;
    let m = migrate_v1(&v1);
    for w in &m.warnings {
        eprintln!("warning: {w}");
    }
    let bank = args.out.join("bank.json");
    let master = args
        .out
        .join("masters")
        .join(format!("{}.json", m.master.id));
    super::write_json(&bank, &m.bank)?;
    super::write_json(&master, &m.master)?;
    println!("{}\n{}", bank.display(), master.display());
    Ok(())
}
