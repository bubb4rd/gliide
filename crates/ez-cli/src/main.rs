//! `ez`: easyshmeezy's development CLI.

mod cmd;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ez", about = "easyshmeezy development CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Turn an ATS Fit master.json (v1) into a bank and an `swe` master.
    Migrate(cmd::migrate::Args),
    /// Resolve a master against a bank and render it: PDF, SVG and PNG pages, metrics.
    Render(cmd::render::Args),
    /// Write the bank and master JSON Schemas.
    Schema(cmd::schema::Args),
    /// Read a posting, extract its requirements, and score a master against it.
    Score(cmd::score::ScoreArgs),
    /// Score every master in a folder against a posting, best first.
    Rank(cmd::score::RankArgs),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Migrate(args) => cmd::migrate::run(args),
        Command::Render(args) => cmd::render::run(args),
        Command::Schema(args) => cmd::schema::run(args),
        Command::Score(args) => cmd::score::score_cmd(args),
        Command::Rank(args) => cmd::score::rank_cmd(args),
    }
}
