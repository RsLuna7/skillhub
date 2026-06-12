use skillhub::config::{AppConfig, McpConfig};
use skillhub::db::Database;
use skillhub::skill::{RiskLevel, Skill};
use std::fs;

fn temp_db(temp: &tempfile::TempDir) -> (AppConfig, Database) {
    let cfg = AppConfig {
        data_dir: temp.path().join("data").to_string_lossy().to_string(),
        default_install_dir: temp.path().join("install").to_string_lossy().to_string(),
        scan_roots: Vec::new(),
        mcp: McpConfig {
            max_file_chars: 12000,
        },
        config_path: temp.path().join("config.toml"),
    };
    let db = Database::open(&cfg).unwrap();
    db.migrate().unwrap();
    (cfg, db)
}

#[test]
fn skill_sources_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let (_cfg, db) = temp_db(&temp);
    db.replace_skill_sources(
        "demo",
        &[
            ("project".into(), "/p/demo".into(), 40u8),
            ("claude".into(), "/c/demo".into(), 20u8),
        ],
    )
    .unwrap();
    let mut sources = db.get_skill_sources("demo").unwrap();
    sources.sort_by_key(|s| s.2);
    assert_eq!(sources.len(), 2);
    assert_eq!(
        sources[1],
        ("project".to_string(), "/p/demo".to_string(), 40)
    );
}

#[test]
fn scan_state_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let (_cfg, db) = temp_db(&temp);
    assert_eq!(db.get_scan_signature("/root/a").unwrap(), None);
    db.set_scan_signature("/root/a", "sig-1").unwrap();
    assert_eq!(
        db.get_scan_signature("/root/a").unwrap().as_deref(),
        Some("sig-1")
    );
}

#[test]
fn skill_provenance_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let (_cfg, db) = temp_db(&temp);
    let skill = Skill {
        id: "demo".into(),
        name: "Demo".into(),
        summary: "Demo summary".into(),
        description: "Demo description".into(),
        install_path: temp.path().join("demo"),
        source_type: "local".into(),
        source_url: None,
        entry_file: Some("SKILL.md".into()),
        readme_file: None,
        has_scripts: false,
        required_env: Vec::new(),
        tags: Vec::new(),
        detected_capabilities: Vec::new(),
        risk_level: RiskLevel::Low,
        last_scanned_at: "now".into(),
        source_agent: "project".into(),
        source_root: temp.path().to_string_lossy().to_string(),
    };

    db.upsert_skill(&skill, &[], &[]).unwrap();
    let loaded = db.get_skill("demo").unwrap().unwrap();
    assert_eq!(loaded.source_agent, "project");
    assert_eq!(loaded.source_root, temp.path().to_string_lossy());
}

fn write_skill(root: &std::path::Path, id: &str, name: &str) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: demo\n---\n# {name}\n"),
    )
    .unwrap();
}

#[test]
fn scan_dedupes_by_priority_and_records_all_sources() {
    let temp = tempfile::tempdir().unwrap();
    let (cfg, db) = temp_db(&temp);

    let global = temp.path().join("global");
    let project = temp.path().join("project");
    write_skill(&global, "shared", "Shared Global");
    write_skill(&project, "shared", "Shared Project");

    let roots = vec![
        skillhub::providers::DiscoveredRoot {
            agent: "claude".into(),
            path: global.clone(),
            priority: skillhub::providers::priority::USER_GLOBAL,
        },
        skillhub::providers::DiscoveredRoot {
            agent: "project".into(),
            path: project.clone(),
            priority: skillhub::providers::priority::PROJECT,
        },
    ];

    let report = skillhub::scan::scan_roots(&cfg, &db, &roots, true).unwrap();
    assert_eq!(report.skills_indexed, 1);

    let skill = db.get_skill("shared").unwrap().unwrap();
    assert_eq!(skill.name, "Shared Project");
    assert_eq!(skill.source_agent, "project");

    let sources = db.get_skill_sources("shared").unwrap();
    assert_eq!(sources.len(), 2);
}

#[test]
fn scan_rejects_folders_without_valid_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let (cfg, db) = temp_db(&temp);
    let root = temp.path().join("root");
    let dir = root.join("not-a-skill");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("README.md"),
        "This mentions a skill and an agent.\n",
    )
    .unwrap();

    let roots = vec![skillhub::providers::DiscoveredRoot {
        agent: "generic".into(),
        path: root,
        priority: skillhub::providers::priority::USER_GLOBAL,
    }];
    let report = skillhub::scan::scan_roots(&cfg, &db, &roots, true).unwrap();
    assert_eq!(report.skills_indexed, 0);
}
