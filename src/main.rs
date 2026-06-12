use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = skillhub::cli::Cli::parse();
    skillhub::cli::run(cli)
}
