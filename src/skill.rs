use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub install_path: PathBuf,
    pub source_type: String,
    pub source_url: Option<String>,
    pub entry_file: Option<String>,
    pub readme_file: Option<String>,
    pub has_scripts: bool,
    pub required_env: Vec<String>,
    pub tags: Vec<String>,
    pub risk_level: RiskLevel,
    pub last_scanned_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFile {
    pub skill_id: String,
    pub relative_path: String,
    pub file_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCommand {
    pub skill_id: String,
    pub runtime: String,
    pub command: String,
    pub args: Vec<String>,
    pub description: String,
    pub source_file: String,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
        }
    }
}

impl RiskLevel {
    pub fn parse(value: &str) -> Self {
        match value {
            "high" => Self::High,
            "medium" => Self::Medium,
            _ => Self::Low,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}
