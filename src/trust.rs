use crate::db::Database;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrustStatus {
    Trusted,
    Untrusted,
    Blocked,
}

impl TrustStatus {
    pub fn parse(value: &str) -> Self {
        match value {
            "trusted" => Self::Trusted,
            "blocked" => Self::Blocked,
            _ => Self::Untrusted,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::Untrusted => "untrusted",
            Self::Blocked => "blocked",
        }
    }
}

impl Display for TrustStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustRecord {
    pub skill_id: String,
    pub status: TrustStatus,
    pub reason: Option<String>,
    pub decided_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Visible,
    Blocked,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Blocked => "blocked",
        }
    }
}

impl Display for Visibility {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub fn visibility_for(status: &TrustStatus) -> Visibility {
    match status {
        TrustStatus::Blocked => Visibility::Blocked,
        _ => Visibility::Visible,
    }
}

pub fn trust_status(db: &Database, skill_id: &str) -> Result<TrustStatus> {
    Ok(db
        .get_trust(skill_id)?
        .map(|record| record.status)
        .unwrap_or(TrustStatus::Untrusted))
}

pub fn is_blocked(db: &Database, skill_id: &str) -> Result<bool> {
    Ok(trust_status(db, skill_id)? == TrustStatus::Blocked)
}

pub fn allow(db: &Database, skill_id: &str) -> Result<TrustRecord> {
    set_status(db, skill_id, TrustStatus::Trusted, None)
}

pub fn block(db: &Database, skill_id: &str, reason: Option<String>) -> Result<TrustRecord> {
    set_status(db, skill_id, TrustStatus::Blocked, reason)
}

pub fn reset(db: &Database, skill_id: &str) -> Result<()> {
    require_skill(db, skill_id)?;
    db.clear_trust(skill_id)
}

pub fn print_list(db: &Database, json: bool) -> Result<()> {
    let mut records = Vec::new();
    for skill in db.list_skills()? {
        let record = db.get_trust(&skill.id)?.unwrap_or(TrustRecord {
            skill_id: skill.id.clone(),
            status: TrustStatus::Untrusted,
            reason: None,
            decided_at: String::new(),
        });
        records.push(record);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&records)?);
        return Ok(());
    }
    println!("{:<24} {:<10} {:<8} Reason", "ID", "Trust", "MCP");
    for record in records {
        println!(
            "{:<24} {:<10} {:<8} {}",
            record.skill_id,
            record.status.as_str(),
            visibility_for(&record.status).as_str(),
            record.reason.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}

fn set_status(
    db: &Database,
    skill_id: &str,
    status: TrustStatus,
    reason: Option<String>,
) -> Result<TrustRecord> {
    require_skill(db, skill_id)?;
    let record = TrustRecord {
        skill_id: skill_id.to_string(),
        status,
        reason,
        decided_at: chrono::Utc::now().to_rfc3339(),
    };
    db.set_trust(&record)?;
    Ok(record)
}

fn require_skill(db: &Database, skill_id: &str) -> Result<()> {
    if db.get_skill(skill_id)?.is_none() {
        bail!("skill not found: {skill_id}");
    }
    Ok(())
}
