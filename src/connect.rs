use anyhow::{Result, bail};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct ConnectReport {
    pub agent: String,
    pub config_path: PathBuf,
    pub changed: bool,
    pub backup_path: Option<PathBuf>,
}

pub fn connect(home: &Path, agent: &str, command: &str, dry_run: bool) -> Result<ConnectReport> {
    match agent {
        "codex" => connect_codex(home, command, dry_run),
        "claude" => connect_claude(home, command, dry_run),
        "cursor" => connect_cursor(home, command, dry_run),
        other => bail!("unknown agent: {other}; expected codex, claude, or cursor"),
    }
}

fn connect_codex(home: &Path, command: &str, dry_run: bool) -> Result<ConnectReport> {
    let config_path = home.join(".codex").join("config.toml");
    let existing = read_existing(&config_path)?;
    if existing.contains("[mcp_servers.skillhub]") {
        return Ok(unchanged("codex", config_path));
    }
    let mut content = existing.clone();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&format!(
        "\n[mcp_servers.skillhub]\ncommand = \"{}\"\nargs = [\"mcp\"]\n",
        command.replace('\\', "\\\\")
    ));
    write_with_backup("codex", &config_path, &existing, &content, dry_run)
}

fn connect_claude(home: &Path, command: &str, dry_run: bool) -> Result<ConnectReport> {
    merge_mcp_json("claude", &home.join(".claude.json"), command, dry_run)
}

fn connect_cursor(home: &Path, command: &str, dry_run: bool) -> Result<ConnectReport> {
    merge_mcp_json(
        "cursor",
        &home.join(".cursor").join("mcp.json"),
        command,
        dry_run,
    )
}

fn merge_mcp_json(
    agent: &str,
    config_path: &Path,
    command: &str,
    dry_run: bool,
) -> Result<ConnectReport> {
    let existing = read_existing(config_path)?;
    let mut root: serde_json::Value = if existing.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&existing).map_err(|err| {
            anyhow::anyhow!(
                "{} is not valid JSON ({err}); fix it manually or move it aside",
                config_path.display()
            )
        })?
    };
    if root["mcpServers"]["skillhub"].is_object() {
        return Ok(unchanged(agent, config_path.to_path_buf()));
    }
    let Some(object) = root.as_object_mut() else {
        bail!("{} is not a JSON object", config_path.display());
    };
    let servers = object
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    if !servers.is_object() {
        bail!(
            "mcpServers in {} is not a JSON object",
            config_path.display()
        );
    }
    servers["skillhub"] = serde_json::json!({ "command": command, "args": ["mcp"] });
    let content = format!(
        "{}
",
        serde_json::to_string_pretty(&root)?
    );
    write_with_backup(agent, config_path, &existing, &content, dry_run)
}

fn read_existing(path: &Path) -> Result<String> {
    if path.exists() {
        Ok(fs::read_to_string(path)?)
    } else {
        Ok(String::new())
    }
}

fn unchanged(agent: &str, config_path: PathBuf) -> ConnectReport {
    ConnectReport {
        agent: agent.to_string(),
        config_path,
        changed: false,
        backup_path: None,
    }
}

fn write_with_backup(
    agent: &str,
    config_path: &Path,
    existing: &str,
    content: &str,
    dry_run: bool,
) -> Result<ConnectReport> {
    let backup_path = if existing.is_empty() {
        None
    } else {
        Some(PathBuf::from(format!("{}.bak", config_path.display())))
    };
    if !dry_run {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(backup) = &backup_path {
            fs::write(backup, existing)?;
        }
        fs::write(config_path, content)?;
    }
    Ok(ConnectReport {
        agent: agent.to_string(),
        config_path: config_path.to_path_buf(),
        changed: true,
        backup_path,
    })
}
