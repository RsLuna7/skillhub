use skillhub::config::{AppConfig, McpConfig};
use skillhub::db::Database;

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
