use crate::config::AppConfig;
use crate::skill::{RiskLevel, Skill, SkillCommand, SkillFile};
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
            "#,
        )?;
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
                readme_file, has_scripts, required_env_json, tags_json, risk_level,
                last_scanned_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
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
        Ok(())
    }

    pub fn list_skills(&self) -> Result<Vec<Skill>> {
        let mut stmt = self.conn.prepare("SELECT * FROM skills ORDER BY id")?;
        let rows = stmt.query_map([], skill_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn search_skills(&self, query: &str) -> Result<Vec<Skill>> {
        let pattern = format!("%{}%", query.to_lowercase());
        let mut stmt = self.conn.prepare(
            r#"
            SELECT * FROM skills
            WHERE lower(id) LIKE ?1 OR lower(name) LIKE ?1 OR lower(summary) LIKE ?1
               OR lower(description) LIKE ?1 OR lower(tags_json) LIKE ?1
            ORDER BY id
            "#,
        )?;
        let rows = stmt.query_map([pattern], skill_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
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
}

fn skill_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Skill> {
    let required_env_json: String = row.get("required_env_json")?;
    let tags_json: String = row.get("tags_json")?;
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
        risk_level: RiskLevel::parse(&risk),
        last_scanned_at: row.get("last_scanned_at")?,
    })
}
