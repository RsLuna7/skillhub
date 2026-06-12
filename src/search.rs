use crate::db::Database;
use anyhow::{Result, bail};

pub fn print_list(db: &Database) -> Result<()> {
    let skills = db.list_skills()?;
    println!("{:<24} {:<28} {:<8} Path", "ID", "Name", "Risk");
    for skill in skills {
        println!(
            "{:<24} {:<28} {:<8} {}",
            truncate(&skill.id, 23),
            truncate(&skill.name, 27),
            skill.risk_level,
            skill.install_path.display()
        );
    }
    Ok(())
}

pub fn print_search(db: &Database, query: &str) -> Result<()> {
    let skills = db.search_skills(query)?;
    if skills.is_empty() {
        println!("No skills matched {query:?}");
        return Ok(());
    }
    println!("{:<24} {:<28} {:<8} Summary", "ID", "Name", "Risk");
    for skill in skills {
        println!(
            "{:<24} {:<28} {:<8} {}",
            truncate(&skill.id, 23),
            truncate(&skill.name, 27),
            skill.risk_level,
            truncate(&skill.summary, 80)
        );
    }
    Ok(())
}

pub fn print_show(db: &Database, skill_id: &str) -> Result<()> {
    let Some(skill) = db.get_skill(skill_id)? else {
        bail!("skill not found: {skill_id}");
    };
    println!("ID: {}", skill.id);
    println!("Name: {}", skill.name);
    println!("Summary: {}", skill.summary);
    println!("Path: {}", skill.install_path.display());
    let source = db
        .get_install_source(skill_id)?
        .or(skill.source_url)
        .unwrap_or_else(|| skill.source_type.clone());
    println!("Source: {}", source);
    println!(
        "Entry: {}",
        skill.entry_file.unwrap_or_else(|| "(none)".into())
    );
    println!(
        "README: {}",
        skill.readme_file.unwrap_or_else(|| "(none)".into())
    );
    println!("Risk: {}", skill.risk_level);
    println!(
        "Required env: {}",
        if skill.required_env.is_empty() {
            "(none)".into()
        } else {
            skill.required_env.join(", ")
        }
    );
    println!("Files:");
    for file in db.get_files(skill_id)? {
        println!("  - {} ({})", file.relative_path, file.file_type);
    }
    println!("Commands:");
    let commands = db.get_commands(skill_id)?;
    if commands.is_empty() {
        println!("  (none)");
    } else {
        for command in commands {
            println!("  - [{}] {}", command.runtime, command.command);
        }
    }
    Ok(())
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut out = value
        .chars()
        .take(max.saturating_sub(1))
        .collect::<String>();
    out.push('.');
    out
}
