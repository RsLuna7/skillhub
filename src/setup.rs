use crate::config::AppConfig;
use crate::db::Database;
use crate::{doctor, scan};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn run_setup(print_agent_instructions: bool) -> Result<()> {
    let cfg = AppConfig::load_or_init()?;
    let db = Database::open(&cfg)?;
    db.migrate()?;
    let report = scan::scan_all(&cfg, &db)?;
    let doctor = doctor::doctor_global(&cfg)?;
    let command = skillhub_command();

    println!("SkillHub setup complete");
    println!("Config: {}", cfg.config_path.display());
    println!("Index: {}", cfg.index_path().display());
    println!(
        "Scanned {} roots, indexed {} skills",
        report.roots_scanned, report.skills_indexed
    );
    println!("Doctor status: {}", doctor.status);
    println!();
    println!("MCP config:");
    println!("{}", mcp_config_json(&command));
    println!();
    println!("Codex:");
    println!(
        "  codex mcp add skillhub -- {} mcp",
        shell_display(&command)
    );
    println!();
    println!("Claude/Cursor:");
    println!("  Add the MCP config above, then restart the agent.");
    println!();
    println!("Next:");
    println!("  skillhub install anysearch-ai/anysearch-skill");
    println!("  skillhub search \"web search\"");
    if print_agent_instructions {
        println!();
        println!("{}", agent_instructions(None));
    }
    Ok(())
}

pub fn agent_instructions(agent: Option<&str>) -> String {
    let prefix = match agent.unwrap_or("generic").to_lowercase().as_str() {
        "codex" => "Codex instruction:",
        "claude" => "Claude instruction:",
        "cursor" => "Cursor instruction:",
        _ => "Agent instruction:",
    };
    format!(
        "{prefix}\nWhen the user asks for a tool, skill, workflow, local capability, or reusable agent behavior, first call skillhub.search_skills before inventing a new solution.\nUse skillhub.get_skill for summaries, skillhub.get_skill_file for full instructions, and skillhub.get_skill_commands only to inspect commands. Do not execute commands unless the user explicitly approves outside SkillHub."
    )
}

fn mcp_config_json(command: &Path) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "skillhub": {
                "command": command.to_string_lossy(),
                "args": ["mcp"]
            }
        }
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

fn skillhub_command() -> PathBuf {
    if command_on_path("skillhub") {
        return PathBuf::from("skillhub");
    }
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("skillhub"))
}

pub(crate) fn command_on_path(command: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|path| {
        let candidate = path.join(command);
        candidate.exists() || candidate.with_extension("exe").exists()
    })
}

fn shell_display(path: &Path) -> String {
    let value = path.to_string_lossy();
    if value.contains(' ') {
        format!("\"{value}\"")
    } else {
        value.to_string()
    }
}
