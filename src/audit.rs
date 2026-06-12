use crate::db::Database;
use crate::skill::Skill;
use anyhow::{Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::path::Path;

pub const RULES_VERSION: &str = "v1";

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
    let is_env_example = Path::new(relative_path)
        .file_name()
        .map(|name| name.to_string_lossy().ends_with(".env.example"))
        .unwrap_or(false);
    for (index, line) in content.lines().enumerate() {
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
                    severity: rule.severity.clone(),
                    file: relative_path.to_string(),
                    line: index + 1,
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
