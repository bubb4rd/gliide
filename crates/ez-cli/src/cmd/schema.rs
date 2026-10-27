use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    /// Folder for bank.schema.json, master.schema.json and story.schema.json.
    #[arg(long, default_value = "schemas")]
    out: PathBuf,
}

pub fn run(args: Args) -> anyhow::Result<()> {
    for (name, schema) in [
        ("bank", resume_core::bank_schema()),
        ("master", resume_core::master_schema()),
        ("story", resume_core::story_schema()),
    ] {
        let path = args.out.join(format!("{name}.schema.json"));
        super::write_json(&path, &schema)?;
        println!("{}", path.display());
    }
    Ok(())
}
