use crate::config::AppConfig;
use crate::db::Database;
use crate::skill::{RiskLevel, Skill, SkillCommand, SkillFile};
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct ScanReport {
    pub roots_scanned: usize,
    pub skills_indexed: usize,
}

type InspectedSkill = (Skill, Vec<SkillFile>, Vec<SkillCommand>);

pub fn scan_all(cfg: &AppConfig, db: &Database) -> Result<ScanReport> {
    let mut roots_scanned = 0;
    let mut skills_indexed = 0;
    for root in cfg.expanded_scan_roots() {
        if !root.exists() {
            continue;
        }
        roots_scanned += 1;
        for dir in candidate_dirs(&root) {
            if let Some((skill, files, commands)) = inspect_skill_dir(&dir)? {
                db.upsert_skill(&skill, &files, &commands)?;
                skills_indexed += 1;
            }
        }
    }
    Ok(ScanReport {
        roots_scanned,
        skills_indexed,
    })
}

fn candidate_dirs(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .max_depth(3)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !matches!(name.as_ref(), ".git" | "node_modules" | "target" | ".venv")
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .collect()
}

fn inspect_skill_dir(dir: &Path) -> Result<Option<InspectedSkill>> {
    let skill_md = dir.join("SKILL.md");
    let skill_yaml = dir.join("skill.yaml");
    let readme = dir.join("README.md");
    let readme_text = read_if_exists(&readme)?;
    let has_readme_keyword = readme_text
        .as_deref()
        .map(|text| {
            let lower = text.to_lowercase();
            lower.contains("skill") || lower.contains("agent") || lower.contains("scripts")
        })
        .unwrap_or(false);

    if !skill_md.exists() && !skill_yaml.exists() && !has_readme_keyword {
        return Ok(None);
    }

    let skill_text = read_if_exists(&skill_md)?;
    let id = normalize_id(
        dir.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .as_ref(),
    );
    let name = skill_text
        .as_deref()
        .and_then(|text| extract_frontmatter_field(text, "name"))
        .or_else(|| {
            readme_text
                .as_deref()
                .and_then(|text| extract_frontmatter_field(text, "name"))
        })
        .or_else(|| skill_text.as_deref().and_then(extract_heading))
        .or_else(|| readme_text.as_deref().and_then(extract_heading))
        .unwrap_or_else(|| id.clone());
    let summary = skill_text
        .as_deref()
        .and_then(extract_frontmatter_description)
        .or_else(|| readme_text.as_deref().and_then(extract_first_paragraph))
        .or_else(|| skill_text.as_deref().and_then(extract_first_paragraph))
        .unwrap_or_else(|| "No summary available.".to_string());
    let description = readme_text
        .as_deref()
        .and_then(extract_first_paragraph)
        .unwrap_or_else(|| summary.clone());
    let scripts = script_files(dir);
    let required_env = detect_env(
        dir,
        skill_text.as_deref().unwrap_or(""),
        readme_text.as_deref().unwrap_or(""),
    )?;
    let risk_level = infer_risk(
        &scripts,
        &required_env,
        skill_text.as_deref().unwrap_or(""),
        readme_text.as_deref().unwrap_or(""),
    );
    let detected_capabilities = detect_capabilities(
        dir,
        &scripts,
        skill_text.as_deref().unwrap_or(""),
        readme_text.as_deref().unwrap_or(""),
    );
    let files = collect_files(dir, &id);
    let commands = scripts
        .iter()
        .map(|script| command_for_script(&id, dir, script, &risk_level))
        .collect::<Result<Vec<_>>>()?;

    let skill = Skill {
        id,
        name,
        summary,
        description,
        install_path: dir.to_path_buf(),
        source_type: if dir.join(".git").exists() {
            "git".into()
        } else {
            "local".into()
        },
        source_url: None,
        entry_file: skill_md.exists().then(|| "SKILL.md".to_string()),
        readme_file: readme.exists().then(|| "README.md".to_string()),
        has_scripts: !scripts.is_empty(),
        required_env,
        tags: Vec::new(),
        detected_capabilities,
        risk_level,
        last_scanned_at: chrono::Utc::now().to_rfc3339(),
    };

    Ok(Some((skill, files, commands)))
}

fn read_if_exists(path: &Path) -> Result<Option<String>> {
    if path.exists() {
        Ok(Some(fs::read_to_string(path).with_context(|| {
            format!("failed to read {}", path.display())
        })?))
    } else {
        Ok(None)
    }
}

fn extract_heading(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.strip_prefix("# ")
            .map(|value| value.trim().to_string())
    })
}

fn extract_frontmatter_description(text: &str) -> Option<String> {
    extract_frontmatter_field(text, "description")
}

fn extract_frontmatter_field(text: &str, field: &str) -> Option<String> {
    if !text.starts_with("---") {
        return None;
    }
    let end = text[3..].find("---")? + 3;
    let prefix = format!("{field}:");
    text[3..end]
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .map(|value| value.trim().trim_matches('"').to_string())
}

fn extract_first_paragraph(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with('#') && !line.starts_with("---"))
        .find(|line| !line.contains(':') || line.split_whitespace().count() > 3)
        .map(|line| line.chars().take(300).collect())
}

fn script_files(dir: &Path) -> Vec<PathBuf> {
    let scripts_dir = dir.join("scripts");
    if !scripts_dir.exists() {
        return Vec::new();
    }
    WalkDir::new(scripts_dir)
        .max_depth(2)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("py" | "js" | "ps1" | "sh")
            )
        })
        .collect()
}

fn detect_env(dir: &Path, skill_text: &str, readme_text: &str) -> Result<Vec<String>> {
    let mut env = BTreeSet::new();
    let regex = Regex::new(r"\b[A-Z][A-Z0-9_]*(?:API_KEY|TOKEN|SECRET|KEY)\b")?;
    for text in [skill_text, readme_text] {
        for mat in regex.find_iter(text) {
            env.insert(mat.as_str().to_string());
        }
    }
    let env_example = dir.join(".env.example");
    if env_example.exists() {
        for line in fs::read_to_string(env_example)?.lines() {
            if let Some((key, _)) = line.split_once('=') {
                let key = key.trim();
                if !key.is_empty() && !key.starts_with('#') {
                    env.insert(key.to_string());
                }
            }
        }
    }
    Ok(env.into_iter().collect())
}

fn infer_risk(
    scripts: &[PathBuf],
    env: &[String],
    skill_text: &str,
    readme_text: &str,
) -> RiskLevel {
    let lower = format!("{skill_text}\n{readme_text}").to_lowercase();
    let high_markers = [
        "delete",
        "overwrite",
        "deploy",
        "git push",
        "shell",
        "browser account",
        "solidworks",
        "docker",
    ];
    if high_markers.iter().any(|marker| lower.contains(marker)) {
        return RiskLevel::High;
    }
    if !scripts.is_empty() || !env.is_empty() || lower.contains("http") || lower.contains("network")
    {
        return RiskLevel::Medium;
    }
    RiskLevel::Low
}

fn detect_capabilities(
    dir: &Path,
    scripts: &[PathBuf],
    skill_text: &str,
    readme_text: &str,
) -> Vec<String> {
    let mut capabilities = BTreeSet::new();
    let script_names = scripts
        .iter()
        .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
        .collect::<Vec<_>>()
        .join(" ");
    let haystack = format!(
        "{}\n{}\n{}\n{}",
        dir.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default(),
        skill_text,
        readme_text,
        script_names
    )
    .to_lowercase();

    if contains_any(
        &haystack,
        &[
            "web search",
            "search engine",
            "anysearch",
            "browser search",
            "联网",
            "搜索",
        ],
    ) {
        capabilities.insert("web_search".to_string());
    }
    if contains_any(
        &haystack,
        &["file", "filesystem", "read file", "write file", "文件"],
    ) {
        capabilities.insert("file_tools".to_string());
    }
    if contains_any(
        &haystack,
        &["writing", "documentation", "report", "文档", "写作"],
    ) {
        capabilities.insert("writing".to_string());
    }
    if contains_any(
        &haystack,
        &["code generation", "coding", "programming", "代码"],
    ) {
        capabilities.insert("coding".to_string());
    }
    if contains_any(&haystack, &["browser", "playwright", "chrome", "浏览器"]) {
        capabilities.insert("browser".to_string());
    }
    if contains_any(&haystack, &["pdf"]) {
        capabilities.insert("pdf".to_string());
    }
    if contains_any(&haystack, &["spreadsheet", "excel", "xlsx", "csv", "表格"]) {
        capabilities.insert("spreadsheet".to_string());
    }
    if contains_any(&haystack, &["image", "png", "jpg", "jpeg", "图片", "图像"]) {
        capabilities.insert("image".to_string());
    }
    if contains_any(&haystack, &["cad", "solidworks", "step", "机械"]) {
        capabilities.insert("cad".to_string());
    }
    if contains_any(
        &haystack,
        &["deploy", "release", "github actions", "发布", "部署"],
    ) {
        capabilities.insert("deployment".to_string());
    }
    capabilities.into_iter().collect()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    let tokens = haystack
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|part| !part.is_empty())
        .collect::<BTreeSet<_>>();
    needles.iter().any(|needle| {
        if needle.is_ascii() && !needle.contains(' ') {
            tokens.contains(needle)
        } else {
            haystack.contains(needle)
        }
    })
}

fn collect_files(dir: &Path, skill_id: &str) -> Vec<SkillFile> {
    let mut files = Vec::new();
    for name in [
        "SKILL.md",
        "README.md",
        "runtime.conf",
        ".env.example",
        "skill.yaml",
    ] {
        if dir.join(name).exists() {
            files.push(SkillFile {
                skill_id: skill_id.to_string(),
                relative_path: name.to_string(),
                file_type: name.to_string(),
            });
        }
    }
    for script in script_files(dir) {
        if let Ok(relative) = script.strip_prefix(dir) {
            files.push(SkillFile {
                skill_id: skill_id.to_string(),
                relative_path: relative.to_string_lossy().replace('\\', "/"),
                file_type: "script".to_string(),
            });
        }
    }
    files
}

fn command_for_script(
    skill_id: &str,
    root: &Path,
    script: &Path,
    risk: &RiskLevel,
) -> Result<SkillCommand> {
    let relative = script
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/");
    let runtime = match script.extension().and_then(|ext| ext.to_str()) {
        Some("py") => "python",
        Some("js") => "node",
        Some("ps1") => "powershell",
        Some("sh") => "bash",
        _ => "unknown",
    };
    let command = match runtime {
        "python" => format!("python {}", script.display()),
        "node" => format!("node {}", script.display()),
        "powershell" => format!(
            "powershell -ExecutionPolicy Bypass -File {}",
            script.display()
        ),
        "bash" => format!("bash {}", script.display()),
        _ => script.display().to_string(),
    };
    Ok(SkillCommand {
        skill_id: skill_id.to_string(),
        runtime: runtime.to_string(),
        command,
        args: vec!["--help".to_string()],
        description: format!("Recommended command for {relative}; SkillHub does not execute it."),
        source_file: relative,
        risk_level: risk.clone(),
    })
}

fn normalize_id(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
