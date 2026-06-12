use crate::db::Database;
use crate::skill::RiskLevel;
use anyhow::{Result, bail};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct DryRunReport {
    pub skill_id: String,
    pub command_index: usize,
    pub command: String,
    pub runtime: String,
    pub cwd: PathBuf,
    pub required_env: Vec<String>,
    pub missing_env: Vec<String>,
    pub risk_level: RiskLevel,
    pub will_execute: bool,
}

pub fn print_run(db: &Database, skill_id: &str, command_index: usize, dry_run: bool) -> Result<()> {
    if !dry_run {
        bail!("SkillHub only supports --dry-run. It does not execute skill commands.");
    }
    let report = dry_run_report(db, skill_id, command_index)?;
    println!("Dry run");
    println!("  skill_id: {}", report.skill_id);
    println!("  command_index: {}", report.command_index);
    println!("  command: {}", report.command);
    println!("  runtime: {}", report.runtime);
    println!("  cwd: {}", report.cwd.display());
    println!("  required_env: {}", display_list(&report.required_env));
    println!("  missing_env: {}", display_list(&report.missing_env));
    println!("  risk_level: {}", report.risk_level);
    println!("  will_execute: {}", report.will_execute);
    Ok(())
}

pub fn dry_run_report(db: &Database, skill_id: &str, command_index: usize) -> Result<DryRunReport> {
    if command_index == 0 {
        bail!("command-index is 1-based; use 1 for the first command");
    }
    let Some(skill) = db.get_skill(skill_id)? else {
        bail!("skill not found: {skill_id}");
    };
    let commands = db.get_commands(skill_id)?;
    let Some(command) = commands.get(command_index - 1) else {
        bail!("command index {command_index} is out of range for skill {skill_id}");
    };
    let missing_env = skill
        .required_env
        .iter()
        .filter(|env| std::env::var(env.as_str()).is_err())
        .cloned()
        .collect::<Vec<_>>();

    Ok(DryRunReport {
        skill_id: skill_id.to_string(),
        command_index,
        command: command.command.clone(),
        runtime: command.runtime.clone(),
        cwd: skill.install_path,
        required_env: skill.required_env,
        missing_env,
        risk_level: command.risk_level.clone(),
        will_execute: false,
    })
}

fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "(none)".to_string()
    } else {
        values.join(", ")
    }
}
