use crate::audit::{AuditReport, AuditStatus};
use crate::config::AppConfig;
use crate::skill::{RiskLevel, Skill, SkillCommand, SkillFile};
use crate::trust::{TrustRecord, TrustStatus};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(cfg: &AppConfig) -> Result<Self> {
        if let Some(parent) = cfg.index_path().parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            conn: Connection::open(cfg.index_path())?,
        })
    }

    pub fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                summary TEXT NOT NULL,
                description TEXT NOT NULL,
                install_path TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_url TEXT,
                entry_file TEXT,
                readme_file TEXT,
                has_scripts INTEGER NOT NULL,
                required_env_json TEXT NOT NULL,
                tags_json TEXT NOT NULL,
                detected_capabilities_json TEXT NOT NULL DEFAULT '[]',
                risk_level TEXT NOT NULL,
                last_scanned_at TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS skill_files (
                skill_id TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                file_type TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (skill_id, relative_path)
            );
            CREATE TABLE IF NOT EXISTS skill_commands (
                id TEXT PRIMARY KEY,
                skill_id TEXT NOT NULL,
                runtime TEXT NOT NULL,
                command TEXT NOT NULL,
                args_json TEXT NOT NULL,
                description TEXT NOT NULL,
                source_file TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scan_roots (
                path TEXT PRIMARY KEY,
                last_scanned_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS doctor_results (
                id TEXT PRIMARY KEY,
                subject TEXT NOT NULL,
                status TEXT NOT NULL,
                report_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS install_sources (
                skill_id TEXT PRIMARY KEY,
                source_url TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS skill_trust (
                skill_id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                reason TEXT,
                decided_at TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS audit_results (
                skill_id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                rules_version TEXT NOT NULL,
                findings_json TEXT NOT NULL,
                audited_at TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS skill_sources (
                skill_id TEXT NOT NULL,
                agent TEXT NOT NULL,
                root TEXT NOT NULL,
                priority INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (skill_id, root)
            );
            CREATE TABLE IF NOT EXISTS scan_state (
                root TEXT PRIMARY KEY,
                signature TEXT NOT NULL,
                last_scanned_at TEXT NOT NULL
            );
            "#,
        )?;
        let _ = self.conn.execute(
            "ALTER TABLE skills ADD COLUMN detected_capabilities_json TEXT NOT NULL DEFAULT '[]'",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE skills ADD COLUMN source_agent TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE skills ADD COLUMN source_root TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = self.conn.execute_batch(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS skill_search_fts USING fts5(
                skill_id UNINDEXED,
                id,
                name,
                summary,
                description,
                tags,
                capabilities
            );
            "#,
        );
        Ok(())
    }

    pub fn upsert_skill(
        &self,
        skill: &Skill,
        files: &[SkillFile],
        commands: &[SkillCommand],
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"
            INSERT INTO skills (
                id, name, summary, description, install_path, source_type, source_url, entry_file,
                readme_file, has_scripts, required_env_json, tags_json, detected_capabilities_json,
                risk_level, last_scanned_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ON CONFLICT(id) DO UPDATE SET
                name=excluded.name,
                summary=excluded.summary,
                description=excluded.description,
                install_path=excluded.install_path,
                source_type=excluded.source_type,
                source_url=excluded.source_url,
                entry_file=excluded.entry_file,
                readme_file=excluded.readme_file,
                has_scripts=excluded.has_scripts,
                required_env_json=excluded.required_env_json,
                tags_json=excluded.tags_json,
                detected_capabilities_json=excluded.detected_capabilities_json,
                risk_level=excluded.risk_level,
                last_scanned_at=excluded.last_scanned_at,
                updated_at=excluded.updated_at
            "#,
            params![
                skill.id,
                skill.name,
                skill.summary,
                skill.description,
                skill.install_path.to_string_lossy(),
                skill.source_type,
                skill.source_url,
                skill.entry_file,
                skill.readme_file,
                if skill.has_scripts { 1 } else { 0 },
                serde_json::to_string(&skill.required_env)?,
                serde_json::to_string(&skill.tags)?,
                serde_json::to_string(&skill.detected_capabilities)?,
                skill.risk_level.as_str(),
                skill.last_scanned_at,
                now,
                now,
            ],
        )?;
        self.conn
            .execute("DELETE FROM skill_files WHERE skill_id = ?1", [&skill.id])?;
        self.conn.execute(
            "DELETE FROM skill_commands WHERE skill_id = ?1",
            [&skill.id],
        )?;
        for file in files {
            self.conn.execute(
                "INSERT INTO skill_files (skill_id, relative_path, file_type, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![file.skill_id, file.relative_path, file.file_type, now, now],
            )?;
        }
        for command in commands {
            self.conn.execute(
                r#"
                INSERT INTO skill_commands
                (id, skill_id, runtime, command, args_json, description, source_file, risk_level, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
                params![
                    format!("{}:{}", command.skill_id, command.source_file),
                    command.skill_id,
                    command.runtime,
                    command.command,
                    serde_json::to_string(&command.args)?,
                    command.description,
                    command.source_file,
                    command.risk_level.as_str(),
                    now,
                    now,
                ],
            )?;
        }
        let _ = self.sync_fts(skill);
        Ok(())
    }

    pub fn list_skills(&self) -> Result<Vec<Skill>> {
        let mut stmt = self.conn.prepare("SELECT * FROM skills ORDER BY id")?;
        let rows = stmt.query_map([], skill_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn search_skills(&self, query: &str) -> Result<Vec<Skill>> {
        if let Ok(results) = self.search_skills_fts(query)
            && !results.is_empty()
        {
            return Ok(results);
        }
        self.search_skills_like(query)
    }

    fn search_skills_fts(&self, query: &str) -> Result<Vec<Skill>> {
        let fts_query = fts_query(query);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT skills.*
            FROM skill_search_fts
            JOIN skills ON skills.id = skill_search_fts.skill_id
            WHERE skill_search_fts MATCH ?1
            ORDER BY bm25(skill_search_fts)
            "#,
        )?;
        let rows = stmt.query_map([fts_query], skill_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn search_skills_like(&self, query: &str) -> Result<Vec<Skill>> {
        let patterns = expanded_terms(query)
            .into_iter()
            .map(|term| format!("%{}%", term.to_lowercase()))
            .collect::<Vec<_>>();
        let pattern_refs = patterns.iter().map(String::as_str).collect::<Vec<_>>();
        let mut stmt = self.conn.prepare(
            r#"
            SELECT * FROM skills
            WHERE lower(id) LIKE ?1 OR lower(name) LIKE ?1 OR lower(summary) LIKE ?1
               OR lower(description) LIKE ?1 OR lower(tags_json) LIKE ?1
               OR lower(detected_capabilities_json) LIKE ?1
            ORDER BY id
            "#,
        )?;
        let mut out = Vec::new();
        for pattern in pattern_refs {
            let rows = stmt.query_map([pattern], skill_from_row)?;
            for row in rows {
                let skill = row?;
                if !out.iter().any(|existing: &Skill| existing.id == skill.id) {
                    out.push(skill);
                }
            }
        }
        Ok(out)
    }

    pub fn get_skill(&self, skill_id: &str) -> Result<Option<Skill>> {
        let mut stmt = self.conn.prepare("SELECT * FROM skills WHERE id = ?1")?;
        stmt.query_row([skill_id], skill_from_row)
            .optional()
            .map_err(Into::into)
    }

    pub fn get_files(&self, skill_id: &str) -> Result<Vec<SkillFile>> {
        let mut stmt = self.conn.prepare(
            "SELECT skill_id, relative_path, file_type FROM skill_files WHERE skill_id = ?1 ORDER BY relative_path",
        )?;
        let rows = stmt.query_map([skill_id], |row| {
            Ok(SkillFile {
                skill_id: row.get(0)?,
                relative_path: row.get(1)?,
                file_type: row.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_commands(&self, skill_id: &str) -> Result<Vec<SkillCommand>> {
        let mut stmt = self.conn.prepare(
            "SELECT skill_id, runtime, command, args_json, description, source_file, risk_level FROM skill_commands WHERE skill_id = ?1 ORDER BY runtime",
        )?;
        let rows = stmt.query_map([skill_id], |row| {
            let args_json: String = row.get(3)?;
            let risk: String = row.get(6)?;
            Ok(SkillCommand {
                skill_id: row.get(0)?,
                runtime: row.get(1)?,
                command: row.get(2)?,
                args: serde_json::from_str(&args_json).unwrap_or_default(),
                description: row.get(4)?,
                source_file: row.get(5)?,
                risk_level: RiskLevel::parse(&risk),
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn record_install_source(&self, skill_id: &str, source_url: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"
            INSERT INTO install_sources (skill_id, source_url, installed_at, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(skill_id) DO UPDATE SET
                source_url=excluded.source_url,
                installed_at=excluded.installed_at,
                updated_at=excluded.updated_at
            "#,
            params![skill_id, source_url, now, now, now],
        )?;
        Ok(())
    }

    pub fn get_install_source(&self, skill_id: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT source_url FROM install_sources WHERE skill_id = ?1",
                [skill_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn set_trust(&self, record: &TrustRecord) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"
            INSERT INTO skill_trust (skill_id, status, reason, decided_at, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(skill_id) DO UPDATE SET
                status=excluded.status,
                reason=excluded.reason,
                decided_at=excluded.decided_at,
                updated_at=excluded.updated_at
            "#,
            params![
                record.skill_id,
                record.status.as_str(),
                record.reason,
                record.decided_at,
                now,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn get_trust(&self, skill_id: &str) -> Result<Option<TrustRecord>> {
        self.conn
            .query_row(
                "SELECT skill_id, status, reason, decided_at FROM skill_trust WHERE skill_id = ?1",
                [skill_id],
                trust_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn clear_trust(&self, skill_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM skill_trust WHERE skill_id = ?1", [skill_id])?;
        Ok(())
    }

    pub fn record_audit(&self, report: &AuditReport) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"
            INSERT INTO audit_results (skill_id, status, rules_version, findings_json, audited_at, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(skill_id) DO UPDATE SET
                status=excluded.status,
                rules_version=excluded.rules_version,
                findings_json=excluded.findings_json,
                audited_at=excluded.audited_at,
                updated_at=excluded.updated_at
            "#,
            params![
                report.skill_id,
                report.status.as_str(),
                report.rules_version,
                serde_json::to_string(&report.findings)?,
                report.audited_at,
                now,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn get_audit(&self, skill_id: &str) -> Result<Option<AuditReport>> {
        self.conn
            .query_row(
                "SELECT skill_id, status, rules_version, findings_json, audited_at FROM audit_results WHERE skill_id = ?1",
                [skill_id],
                audit_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn replace_skill_sources(
        &self,
        skill_id: &str,
        sources: &[(String, String, u8)],
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .execute("DELETE FROM skill_sources WHERE skill_id = ?1", [skill_id])?;
        for (agent, root, priority) in sources {
            self.conn.execute(
                "INSERT INTO skill_sources (skill_id, agent, root, priority, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![skill_id, agent, root, *priority as i64, now],
            )?;
        }
        Ok(())
    }

    pub fn get_skill_sources(&self, skill_id: &str) -> Result<Vec<(String, String, u8)>> {
        let mut stmt = self.conn.prepare(
            "SELECT agent, root, priority FROM skill_sources WHERE skill_id = ?1 ORDER BY priority DESC, root",
        )?;
        let rows = stmt.query_map([skill_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? as u8,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_scan_signature(&self, root: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT signature FROM scan_state WHERE root = ?1",
                [root],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn set_scan_signature(&self, root: &str, signature: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"
            INSERT INTO scan_state (root, signature, last_scanned_at) VALUES (?1, ?2, ?3)
            ON CONFLICT(root) DO UPDATE SET signature=excluded.signature, last_scanned_at=excluded.last_scanned_at
            "#,
            params![root, signature, now],
        )?;
        Ok(())
    }

    fn sync_fts(&self, skill: &Skill) -> Result<()> {
        self.conn.execute(
            "DELETE FROM skill_search_fts WHERE skill_id = ?1",
            [&skill.id],
        )?;
        self.conn.execute(
            r#"
            INSERT INTO skill_search_fts (skill_id, id, name, summary, description, tags, capabilities)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                skill.id,
                skill.id,
                skill.name,
                skill.summary,
                skill.description,
                skill.tags.join(" "),
                skill.detected_capabilities.join(" "),
            ],
        )?;
        Ok(())
    }
}

fn trust_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TrustRecord> {
    let status: String = row.get(1)?;
    Ok(TrustRecord {
        skill_id: row.get(0)?,
        status: TrustStatus::parse(&status),
        reason: row.get(2)?,
        decided_at: row.get(3)?,
    })
}

fn audit_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditReport> {
    let status: String = row.get(1)?;
    let findings_json: String = row.get(3)?;
    Ok(AuditReport {
        skill_id: row.get(0)?,
        status: AuditStatus::parse(&status),
        rules_version: row.get(2)?,
        findings: serde_json::from_str(&findings_json).unwrap_or_default(),
        audited_at: row.get(4)?,
    })
}

fn skill_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Skill> {
    let required_env_json: String = row.get("required_env_json")?;
    let tags_json: String = row.get("tags_json")?;
    let detected_capabilities_json: String = row
        .get("detected_capabilities_json")
        .unwrap_or_else(|_| "[]".to_string());
    let risk: String = row.get("risk_level")?;
    let install_path: String = row.get("install_path")?;
    Ok(Skill {
        id: row.get("id")?,
        name: row.get("name")?,
        summary: row.get("summary")?,
        description: row.get("description")?,
        install_path: PathBuf::from(install_path),
        source_type: row.get("source_type")?,
        source_url: row.get("source_url")?,
        entry_file: row.get("entry_file")?,
        readme_file: row.get("readme_file")?,
        has_scripts: row.get::<_, i64>("has_scripts")? == 1,
        required_env: serde_json::from_str(&required_env_json).unwrap_or_default(),
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        detected_capabilities: serde_json::from_str(&detected_capabilities_json)
            .unwrap_or_default(),
        risk_level: RiskLevel::parse(&risk),
        last_scanned_at: row.get("last_scanned_at")?,
    })
}

fn expanded_terms(query: &str) -> Vec<String> {
    let lower = query.to_lowercase();
    let mut terms = vec![lower.clone()];
    if lower.contains("联网") || lower.contains("搜索") || lower.contains("web") {
        terms.extend(["web_search".into(), "search".into(), "web".into()]);
    }
    if lower.contains("写") || lower.contains("文档") || lower.contains("write") {
        terms.extend(["writing".into(), "documentation".into()]);
    }
    if lower.contains("pdf") {
        terms.push("pdf".into());
    }
    if lower.contains("表格") || lower.contains("excel") || lower.contains("spreadsheet") {
        terms.extend(["spreadsheet".into(), "excel".into()]);
    }
    terms.sort();
    terms.dedup();
    terms
}

fn fts_query(query: &str) -> String {
    expanded_terms(query)
        .into_iter()
        .flat_map(|term| {
            term.split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>()
        .join(" OR ")
}
