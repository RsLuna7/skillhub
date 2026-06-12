use crate::db::Database;
use crate::skill::{AuditInfo, SkillUsageSummary, next_actions};
use crate::trust::{TrustStatus, visibility_for};
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

pub fn print_search(db: &Database, query: &str, json: bool) -> Result<()> {
    let skills = db.search_skills(query)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&skills)?);
        return Ok(());
    }
    if skills.is_empty() {
        println!("No skills matched {query:?}");
        return Ok(());
    }
    println!(
        "{:<24} {:<28} {:<8} {:<20} Summary",
        "ID", "Name", "Risk", "Capabilities"
    );
    for skill in skills {
        println!(
            "{:<24} {:<28} {:<8} {:<20} {}",
            truncate(&skill.id, 23),
            truncate(&skill.name, 27),
            skill.risk_level,
            truncate(&skill.detected_capabilities.join(","), 19),
            truncate(&skill.summary, 80)
        );
    }
    Ok(())
}

pub fn print_show(db: &Database, skill_id: &str, json: bool) -> Result<()> {
    let summary = usage_summary(db, skill_id)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }
    let skill = &summary.skill;
    println!("{} ({})", skill.name, skill.id);
    println!();
    println!("What it does");
    println!("  {}", skill.summary);
    println!();
    println!("Where it is");
    println!("  {}", skill.install_path.display());
    println!("  Source: {}", summary.source);
    println!();
    println!("How to use it");
    println!(
        "  Read {} first, then inspect commands if needed.",
        skill.entry_file.as_deref().unwrap_or("SKILL.md")
    );
    for action in &summary.next_actions {
        println!("  - {action}");
    }
    println!();
    println!("Required env");
    if skill.required_env.is_empty() {
        println!("  (none)");
    } else {
        for env in &skill.required_env {
            println!("  - {env}");
        }
    }
    println!();
    println!("Capabilities");
    if skill.detected_capabilities.is_empty() {
        println!("  (none detected)");
    } else {
        for capability in &skill.detected_capabilities {
            println!("  - {capability}");
        }
    }
    println!();
    println!("Available files");
    for file in &summary.files {
        println!("  - {} ({})", file.relative_path, file.file_type);
    }
    println!();
    println!("Recommended commands");
    if summary.commands.is_empty() {
        println!("  (none)");
    } else {
        for (idx, command) in summary.commands.iter().enumerate() {
            println!("  {}. [{}] {}", idx + 1, command.runtime, command.command);
        }
    }
    println!();
    println!("Safety notes");
    println!("  Risk: {}", skill.risk_level);
    println!("  Trust: {}", summary.trust);
    if let Some(reason) = &summary.trust_reason {
        println!("  Trust reason: {reason}");
    }
    match &summary.audit {
        Some(audit) => println!(
            "  Audit: {} ({} findings, rules {}, {})",
            audit.status, audit.findings, audit.rules_version, audit.audited_at
        ),
        None => println!("  Audit: not audited (run `skillhub audit {}`)", skill.id),
    }
    println!("  MCP visibility: {}", summary.visibility);
    println!(
        "  SkillHub does not execute skill commands. Use `skillhub run {} 1 --dry-run` to preview.",
        skill.id
    );
    Ok(())
}

pub fn usage_summary(db: &Database, skill_id: &str) -> Result<SkillUsageSummary> {
    let Some(skill) = db.get_skill(skill_id)? else {
        bail!("skill not found: {skill_id}");
    };
    let source = db
        .get_install_source(skill_id)?
        .or_else(|| skill.source_url.clone())
        .unwrap_or_else(|| skill.source_type.clone());
    let trust_record = db.get_trust(skill_id)?;
    let trust = trust_record
        .as_ref()
        .map(|record| record.status.clone())
        .unwrap_or(TrustStatus::Untrusted);
    let trust_reason = trust_record.and_then(|record| record.reason);
    let audit = db.get_audit(skill_id)?.map(|report| AuditInfo {
        status: report.status,
        findings: report.findings.len(),
        rules_version: report.rules_version,
        audited_at: report.audited_at,
    });
    Ok(SkillUsageSummary {
        next_actions: next_actions(skill_id),
        files: db.get_files(skill_id)?,
        commands: db.get_commands(skill_id)?,
        source,
        visibility: visibility_for(&trust),
        trust,
        trust_reason,
        audit,
        skill,
    })
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
