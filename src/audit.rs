use crate::db::Database;
use crate::skill::Skill;
use anyhow::{Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::path::Path;

pub const RULES_VERSION: &str = "v2";

const EXCERPT_MAX_CHARS: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuditStatus {
    Pass,
    Warn,
    Fail,
}

impl AuditStatus {
    pub fn parse(value: &str) -> Self {
        match value {
            "fail" => Self::Fail,
            "warn" => Self::Warn,
            _ => Self::Pass,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

impl Display for AuditStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    Low,
    Medium,
    High,
}

impl FindingSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl Display for FindingSeverity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    pub rule: String,
    pub severity: FindingSeverity,
    pub file: String,
    pub line: usize,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub skill_id: String,
    pub status: AuditStatus,
    pub rules_version: String,
    pub findings: Vec<AuditFinding>,
    pub audited_at: String,
}

impl Display for AuditReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{}: {} ({} findings, rules {})",
            self.skill_id,
            self.status,
            self.findings.len(),
            self.rules_version
        )?;
        for finding in &self.findings {
            writeln!(
                f,
                "- {:<26} {:<7} {}:{} {}",
                finding.rule, finding.severity, finding.file, finding.line, finding.excerpt
            )?;
        }
        Ok(())
    }
}

struct AuditRule {
    id: &'static str,
    severity: FindingSeverity,
    pattern: Regex,
    exempt: Option<Regex>,
    redact_excerpt: bool,
    skip_env_example: bool,
}

fn rules() -> Vec<AuditRule> {
    let rule = |id, severity, pattern: &str, redact_excerpt, skip_env_example| AuditRule {
        id,
        severity,
        pattern: Regex::new(pattern).expect("audit rule regex must compile"),
        exempt: None,
        redact_excerpt,
        skip_env_example,
    };
    vec![
        rule(
            "remote-script-execution",
            FindingSeverity::High,
            r"(?i)\b(curl|wget|iwr|invoke-webrequest)\b[^|\n]*\|\s*(sh|bash|zsh|powershell|pwsh|iex)\b",
            false,
            false,
        ),
        rule(
            "destructive-delete",
            FindingSeverity::High,
            r"(?i)\brm\s+-[a-z]*[rf][a-z]*[rf][a-z]*\b|\bremove-item\b.*-(recurse|force)|\bdel\s+/f\b|\brmdir\s+/s\b",
            false,
            false,
        ),
        rule(
            "dynamic-execution",
            FindingSeverity::High,
            r"(?i)\beval\s*\(|\binvoke-expression\b|(?:^|[;|&]\s*)iex\b",
            false,
            false,
        ),
        rule(
            "hardcoded-secret",
            FindingSeverity::High,
            r#"(?i)\b[a-z0-9_]*(api_key|apikey|secret|token|password)\b\s*[:=]\s*['"][^'"]{8,}['"]"#,
            true,
            true,
        ),
        rule(
            "privilege-escalation",
            FindingSeverity::Medium,
            r"(?i)\bsudo\s+",
            false,
            false,
        ),
        rule(
            "obfuscation",
            FindingSeverity::Medium,
            r"(?i)base64\s+(-d|--decode)\b|frombase64string|\bb64decode\b",
            false,
            false,
        ),
        AuditRule {
            id: "insecure-http",
            severity: FindingSeverity::Low,
            pattern: Regex::new(r"(?i)\bhttp://").expect("audit rule regex must compile"),
            exempt: Some(
                Regex::new(r"(?i)\bhttp://(localhost|127\.0\.0\.1)")
                    .expect("audit rule regex must compile"),
            ),
            redact_excerpt: false,
            skip_env_example: false,
        },
    ]
}

pub fn audit_skill(db: &Database, skill_id: &str) -> Result<AuditReport> {
    let Some(skill) = db.get_skill(skill_id)? else {
        bail!("skill not found: {skill_id}");
    };
    let report = build_report(db, &skill)?;
    db.record_audit(&report)?;
    Ok(report)
}

pub fn audit_all(db: &Database) -> Result<Vec<AuditReport>> {
    let mut reports = Vec::new();
    for skill in db.list_skills()? {
        let report = build_report(db, &skill)?;
        db.record_audit(&report)?;
        reports.push(report);
    }
    Ok(reports)
}

fn build_report(db: &Database, skill: &Skill) -> Result<AuditReport> {
    let mut findings = Vec::new();
    let rules = rules();
    let mut files = db.get_files(&skill.id)?;
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    for file in files {
        let path = skill.install_path.join(&file.relative_path);
        if !path.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        audit_file_content(&file.relative_path, &content, &rules, &mut findings);
    }
    Ok(AuditReport {
        skill_id: skill.id.clone(),
        status: status_for(&findings),
        rules_version: RULES_VERSION.to_string(),
        findings,
        audited_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn audit_file_content(
    relative_path: &str,
    content: &str,
    rules: &[AuditRule],
    findings: &mut Vec<AuditFinding>,
) {
    let context = classify_file(relative_path);
    let is_env_example = Path::new(relative_path)
        .file_name()
        .map(|name| name.to_string_lossy().ends_with(".env.example"))
        .unwrap_or(false);
    for (line_number, line) in scannable_lines(&context, content) {
        for rule in rules {
            if rule.skip_env_example && is_env_example {
                continue;
            }
            let exempt = rule
                .exempt
                .as_ref()
                .map(|pattern| pattern.is_match(line))
                .unwrap_or(false);
            if rule.pattern.is_match(line) && !exempt {
                findings.push(AuditFinding {
                    rule: rule.id.to_string(),
                    severity: finding_severity(rule, line, &context),
                    file: relative_path.to_string(),
                    line: line_number,
                    excerpt: if rule.redact_excerpt {
                        "(redacted)".to_string()
                    } else {
                        truncate_excerpt(line)
                    },
                });
            }
        }
    }
}

fn status_for(findings: &[AuditFinding]) -> AuditStatus {
    if findings.iter().any(|f| f.severity == FindingSeverity::High) {
        AuditStatus::Fail
    } else if findings.is_empty() {
        AuditStatus::Pass
    } else {
        AuditStatus::Warn
    }
}

fn truncate_excerpt(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.chars().count() <= EXCERPT_MAX_CHARS {
        return trimmed.to_string();
    }
    let mut out = trimmed.chars().take(EXCERPT_MAX_CHARS).collect::<String>();
    out.push('…');
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileContext {
    Executable,
    Documentation,
    Config,
}

fn classify_file(relative_path: &str) -> FileContext {
    let extension = Path::new(relative_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    match extension.to_ascii_lowercase().as_str() {
        "sh" | "ps1" | "py" | "js" => FileContext::Executable,
        "md" | "markdown" => FileContext::Documentation,
        _ => FileContext::Config,
    }
}

fn scannable_lines<'a>(context: &FileContext, content: &'a str) -> Vec<(usize, &'a str)> {
    match context {
        FileContext::Documentation => fenced_code_lines(content),
        _ => content
            .lines()
            .enumerate()
            .map(|(index, line)| (index + 1, line))
            .collect(),
    }
}

fn fenced_code_lines(content: &str) -> Vec<(usize, &str)> {
    let mut in_fence = false;
    let mut out = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            out.push((index + 1, line));
        }
    }
    out
}

fn downgrade(severity: FindingSeverity) -> FindingSeverity {
    match severity {
        FindingSeverity::High => FindingSeverity::Medium,
        FindingSeverity::Medium | FindingSeverity::Low => FindingSeverity::Low,
    }
}

fn destructive_delete_severity(line: &str) -> FindingSeverity {
    let is_rm = Regex::new(r"(?i)\brm\s+-").expect("audit rule regex must compile");
    let system_target = Regex::new(r#"(?i)\brm\s+(?:-[a-z]+\s+)*["']?(?:/|~|\$home\b|[a-z]:[\/])"#)
        .expect("audit rule regex must compile");
    if !is_rm.is_match(line) || system_target.is_match(line) {
        FindingSeverity::High
    } else {
        FindingSeverity::Medium
    }
}

fn finding_severity(rule: &AuditRule, line: &str, context: &FileContext) -> FindingSeverity {
    let base = if rule.id == "destructive-delete" {
        destructive_delete_severity(line)
    } else {
        rule.severity.clone()
    };
    match context {
        FileContext::Documentation => downgrade(base),
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_file_by_extension() {
        assert!(matches!(
            classify_file("scripts/run.sh"),
            FileContext::Executable
        ));
        assert!(matches!(
            classify_file("scripts/tool.ps1"),
            FileContext::Executable
        ));
        assert!(matches!(
            classify_file("SKILL.md"),
            FileContext::Documentation
        ));
        assert!(matches!(classify_file("runtime.conf"), FileContext::Config));
        assert!(matches!(classify_file(".env.example"), FileContext::Config));
    }

    #[test]
    fn fenced_code_lines_skips_prose_and_keeps_line_numbers() {
        let md = "prose rm -rf /\n```bash\nrm -rf /\n```\nmore prose\n";
        let lines = fenced_code_lines(md);
        assert_eq!(lines, vec![(3, "rm -rf /")]);
    }

    #[test]
    fn fenced_code_lines_handles_tilde_fences() {
        let md = "~~~\necho hi\n~~~\n";
        assert_eq!(fenced_code_lines(md), vec![(2, "echo hi")]);
    }

    #[test]
    fn scannable_lines_uses_all_lines_for_scripts() {
        let lines = scannable_lines(
            &FileContext::Executable,
            "a
b
",
        );
        assert_eq!(lines, vec![(1, "a"), (2, "b")]);
    }

    #[test]
    fn rm_with_relative_or_variable_target_is_medium() {
        assert_eq!(
            destructive_delete_severity("rm -rf node_modules"),
            FindingSeverity::Medium
        );
        assert_eq!(
            destructive_delete_severity("rm -rf \"$APP_DIR\""),
            FindingSeverity::Medium
        );
    }

    #[test]
    fn rm_with_system_target_is_high() {
        assert_eq!(
            destructive_delete_severity("sudo rm -rf /var/data"),
            FindingSeverity::High
        );
        assert_eq!(
            destructive_delete_severity("rm -rf ~/.config"),
            FindingSeverity::High
        );
        assert_eq!(
            destructive_delete_severity("rm -rf $HOME/.cache"),
            FindingSeverity::High
        );
    }

    #[test]
    fn non_rm_deletes_stay_high() {
        assert_eq!(
            destructive_delete_severity("Remove-Item -Recurse -Force C:\\data"),
            FindingSeverity::High
        );
        assert_eq!(
            destructive_delete_severity("del /f important.txt"),
            FindingSeverity::High
        );
    }

    #[test]
    fn documentation_context_downgrades_one_level() {
        assert_eq!(downgrade(FindingSeverity::High), FindingSeverity::Medium);
        assert_eq!(downgrade(FindingSeverity::Medium), FindingSeverity::Low);
        assert_eq!(downgrade(FindingSeverity::Low), FindingSeverity::Low);
    }
}
