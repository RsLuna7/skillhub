use serde_json::{Value, json};
use skillhub::audit::{AuditStatus, FindingSeverity, audit_all, audit_skill};
use skillhub::config::AppConfig;
use skillhub::db::Database;
use skillhub::doctor::{doctor_agent, doctor_agents};
use skillhub::mcp::handle_request;
use skillhub::scan::scan_roots;
use skillhub::search::usage_summary;
use skillhub::trust;
use skillhub::trust::{TrustStatus, Visibility};

fn test_config(temp: &tempfile::TempDir) -> AppConfig {
    let root = std::env::current_dir()
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join("skills");
    AppConfig {
        data_dir: temp.path().join("data").to_string_lossy().to_string(),
        default_install_dir: temp.path().join("skills").to_string_lossy().to_string(),
        scan_roots: vec![root.to_string_lossy().to_string()],
        mcp: skillhub::config::McpConfig {
            max_file_chars: 12_000,
        },
        config_path: temp.path().join("config.toml"),
    }
}

fn scanned_db(temp: &tempfile::TempDir) -> (AppConfig, Database) {
    let cfg = test_config(temp);
    let db = Database::open(&cfg).unwrap();
    db.migrate().unwrap();
    let roots = vec![skillhub::providers::DiscoveredRoot {
        agent: "user-config".into(),
        path: std::path::PathBuf::from(&cfg.scan_roots[0]),
        priority: skillhub::providers::priority::USER_CONFIG,
    }];
    scan_roots(&cfg, &db, &roots, true).unwrap();
    (cfg, db)
}

fn call_tool(cfg: &AppConfig, db: &Database, name: &str, args: Value) -> anyhow::Result<Value> {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": name, "arguments": args }
    });
    let result = handle_request(cfg, db, &req)?;
    let text = result["content"][0]["text"].as_str().unwrap().to_string();
    Ok(serde_json::from_str(&text)?)
}

#[test]
fn audit_flags_risky_fixture_and_persists_report() {
    let temp = tempfile::tempdir().unwrap();
    let (_cfg, db) = scanned_db(&temp);

    let report = audit_skill(&db, "danger-tool").unwrap();
    assert_eq!(report.status, AuditStatus::Fail);
    let rules = report
        .findings
        .iter()
        .map(|finding| finding.rule.as_str())
        .collect::<Vec<_>>();
    assert!(rules.contains(&"remote-script-execution"));
    assert!(rules.contains(&"destructive-delete"));
    assert!(rules.contains(&"privilege-escalation"));
    assert!(rules.contains(&"insecure-http"));
    assert!(
        report
            .findings
            .iter()
            .all(|finding| finding.file == "scripts/cleanup.sh")
    );

    let stored = db.get_audit("danger-tool").unwrap().unwrap();
    assert_eq!(stored.status, AuditStatus::Fail);
    assert_eq!(stored.findings.len(), report.findings.len());
}

#[test]
fn audit_passes_clean_fixture_and_audit_all_covers_everything() {
    let temp = tempfile::tempdir().unwrap();
    let (_cfg, db) = scanned_db(&temp);

    let report = audit_skill(&db, "writing-helper").unwrap();
    assert_eq!(report.status, AuditStatus::Pass);
    assert!(report.findings.is_empty());

    let reports = audit_all(&db).unwrap();
    assert_eq!(reports.len(), 4);
    assert!(reports.iter().any(|r| r.skill_id == "danger-tool"));

    assert!(audit_skill(&db, "missing-skill").is_err());
}

#[test]
fn trust_allow_block_reset_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let (_cfg, db) = scanned_db(&temp);

    assert_eq!(
        trust::trust_status(&db, "anysearch").unwrap(),
        TrustStatus::Untrusted
    );

    trust::allow(&db, "anysearch").unwrap();
    assert_eq!(
        trust::trust_status(&db, "anysearch").unwrap(),
        TrustStatus::Trusted
    );

    trust::block(&db, "anysearch", Some("supply chain review pending".into())).unwrap();
    assert_eq!(
        trust::trust_status(&db, "anysearch").unwrap(),
        TrustStatus::Blocked
    );
    assert!(trust::is_blocked(&db, "anysearch").unwrap());

    trust::reset(&db, "anysearch").unwrap();
    assert_eq!(
        trust::trust_status(&db, "anysearch").unwrap(),
        TrustStatus::Untrusted
    );

    assert!(trust::allow(&db, "missing-skill").is_err());
}

#[test]
fn usage_summary_includes_trust_audit_and_visibility_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let (_cfg, db) = scanned_db(&temp);

    let summary = usage_summary(&db, "danger-tool").unwrap();
    assert_eq!(summary.trust, TrustStatus::Untrusted);
    assert_eq!(summary.visibility, Visibility::Visible);
    assert!(summary.audit.is_none());

    audit_skill(&db, "danger-tool").unwrap();
    trust::block(&db, "danger-tool", Some("audit failed".into())).unwrap();

    let summary = usage_summary(&db, "danger-tool").unwrap();
    assert_eq!(summary.trust, TrustStatus::Blocked);
    assert_eq!(summary.trust_reason.as_deref(), Some("audit failed"));
    assert_eq!(summary.visibility, Visibility::Blocked);
    let audit = summary.audit.unwrap();
    assert_eq!(audit.status, AuditStatus::Fail);
    assert!(audit.findings > 0);
}

#[test]
fn mcp_hides_and_refuses_blocked_skills() {
    let temp = tempfile::tempdir().unwrap();
    let (cfg, db) = scanned_db(&temp);

    let listed = call_tool(&cfg, &db, "skillhub.list_skills", json!({})).unwrap();
    assert_eq!(listed["skills"].as_array().unwrap().len(), 4);

    trust::block(&db, "anysearch", None).unwrap();

    let listed = call_tool(&cfg, &db, "skillhub.list_skills", json!({})).unwrap();
    let ids = listed["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skill| skill["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 3);
    assert!(!ids.contains(&"anysearch".to_string()));

    let results = call_tool(
        &cfg,
        &db,
        "skillhub.search_skills",
        json!({"query": "web search"}),
    )
    .unwrap();
    assert!(results["results"].as_array().unwrap().is_empty());

    for (tool, args) in [
        ("skillhub.get_skill", json!({"skill_id": "anysearch"})),
        (
            "skillhub.get_skill_file",
            json!({"skill_id": "anysearch", "file": "SKILL.md"}),
        ),
        (
            "skillhub.get_skill_commands",
            json!({"skill_id": "anysearch"}),
        ),
        ("skillhub.doctor_skill", json!({"skill_id": "anysearch"})),
    ] {
        let err = call_tool(&cfg, &db, tool, args).unwrap_err();
        assert!(
            err.to_string().contains("blocked by local trust policy"),
            "{tool} should refuse blocked skills, got: {err}"
        );
    }

    trust::reset(&db, "anysearch").unwrap();
    let skill = call_tool(
        &cfg,
        &db,
        "skillhub.get_skill",
        json!({"skill_id": "anysearch"}),
    )
    .unwrap();
    assert_eq!(skill["skill"]["trust"], json!("untrusted"));
    assert_eq!(skill["skill"]["visibility"], json!("visible"));
}

#[test]
fn doctor_agent_reports_integration_state_from_home_dir() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    std::fs::create_dir_all(home.join(".codex").join("skills")).unwrap();
    std::fs::write(
        home.join(".codex").join("config.toml"),
        "[mcp_servers.skillhub]\ncommand = \"skillhub\"\nargs = [\"mcp\"]\n",
    )
    .unwrap();

    let report = doctor_agent(home, "codex").unwrap();
    assert_eq!(report.subject, "agent:codex");
    let check = |name: &str| {
        report
            .checks
            .iter()
            .find(|check| check.name == name)
            .unwrap()
            .status
            .clone()
    };
    assert_eq!(check("config_file"), "ok");
    assert_eq!(check("mcp_entry"), "ok");
    assert_eq!(check("skills_dir"), "ok");

    let cursor = doctor_agent(home, "cursor").unwrap();
    assert_eq!(cursor.subject, "agent:cursor");
    assert!(
        cursor
            .checks
            .iter()
            .any(|check| check.name == "mcp_entry" && check.status == "warning")
    );

    let reports = doctor_agents(home).unwrap();
    assert_eq!(reports.len(), 3);

    assert!(doctor_agent(home, "vscode").is_err());
}

#[test]
fn migrate_upgrades_v02_database_in_place() {
    let temp = tempfile::tempdir().unwrap();
    let cfg = test_config(&temp);
    std::fs::create_dir_all(cfg.index_path().parent().unwrap()).unwrap();
    {
        let conn = rusqlite::Connection::open(cfg.index_path()).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE skills (
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
            INSERT INTO skills VALUES (
                'legacy-skill', 'Legacy Skill', 'A v0.2 skill.', 'A v0.2 skill.',
                '/tmp/legacy-skill', 'local', NULL, 'SKILL.md', NULL, 0,
                '[]', '[]', '[]', 'low', '2026-01-01T00:00:00Z',
                '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'
            );
            "#,
        )
        .unwrap();
    }

    let db = Database::open(&cfg).unwrap();
    db.migrate().unwrap();

    let skills = db.list_skills().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].id, "legacy-skill");

    assert_eq!(
        trust::trust_status(&db, "legacy-skill").unwrap(),
        TrustStatus::Untrusted
    );
    trust::block(&db, "legacy-skill", None).unwrap();
    assert!(trust::is_blocked(&db, "legacy-skill").unwrap());
    assert!(db.get_audit("legacy-skill").unwrap().is_none());
}

#[test]
fn audit_v2_ignores_prose_and_downgrades_doc_code_blocks() {
    let temp = tempfile::tempdir().unwrap();
    let (_cfg, db) = scanned_db(&temp);

    let report = audit_skill(&db, "docs-heavy").unwrap();
    assert_eq!(report.rules_version, "v2");
    assert_eq!(report.status, AuditStatus::Warn);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].rule, "destructive-delete");
    assert_eq!(report.findings[0].severity, FindingSeverity::Low);
}
