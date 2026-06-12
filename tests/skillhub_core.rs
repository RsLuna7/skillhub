use skillhub::config::AppConfig;
use skillhub::db::Database;
use skillhub::run::{dry_run_report, print_run};
use skillhub::scan::scan_roots;
use skillhub::search::usage_summary;
use skillhub::security::safe_skill_file_path;

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

fn scan_fixtures(cfg: &AppConfig, db: &Database) -> anyhow::Result<skillhub::scan::ScanReport> {
    let roots = vec![skillhub::providers::DiscoveredRoot {
        agent: "user-config".into(),
        path: std::path::PathBuf::from(&cfg.scan_roots[0]),
        priority: skillhub::providers::priority::USER_CONFIG,
    }];
    scan_roots(cfg, db, &roots, true)
}

#[test]
fn scan_indexes_fixture_skills_and_commands() {
    let temp = tempfile::tempdir().unwrap();
    let cfg = test_config(&temp);
    let db = Database::open(&cfg).unwrap();
    db.migrate().unwrap();

    let report = scan_fixtures(&cfg, &db).unwrap();
    assert_eq!(report.roots_scanned, 1);
    assert_eq!(report.skills_indexed, 4);

    let anysearch = db.get_skill("anysearch").unwrap().unwrap();
    assert_eq!(anysearch.name, "AnySearch Skill");
    assert!(anysearch.has_scripts);
    assert!(
        anysearch
            .required_env
            .contains(&"ANYSEARCH_API_KEY".to_string())
    );
    assert!(
        anysearch
            .detected_capabilities
            .contains(&"web_search".to_string())
    );

    let commands = db.get_commands("anysearch").unwrap();
    assert_eq!(commands.len(), 2);
    assert!(commands.iter().any(|cmd| cmd.runtime == "python"));
    assert!(commands.iter().any(|cmd| cmd.runtime == "node"));
}

#[test]
fn search_matches_summary_and_name() {
    let temp = tempfile::tempdir().unwrap();
    let cfg = test_config(&temp);
    let db = Database::open(&cfg).unwrap();
    db.migrate().unwrap();
    scan_fixtures(&cfg, &db).unwrap();

    let results = db.search_skills("web search").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "anysearch");

    let cn_results = db.search_skills("联网搜索").unwrap();
    assert_eq!(cn_results.len(), 1);
    assert_eq!(cn_results[0].id, "anysearch");
}

#[test]
fn usage_summary_includes_next_actions_and_commands() {
    let temp = tempfile::tempdir().unwrap();
    let cfg = test_config(&temp);
    let db = Database::open(&cfg).unwrap();
    db.migrate().unwrap();
    scan_fixtures(&cfg, &db).unwrap();

    let summary = usage_summary(&db, "anysearch").unwrap();
    assert_eq!(summary.skill.id, "anysearch");
    assert!(
        summary
            .next_actions
            .iter()
            .any(|action| action.contains("get_skill_file"))
    );
    assert_eq!(summary.commands.len(), 2);
}

#[test]
fn run_is_dry_run_only() {
    let temp = tempfile::tempdir().unwrap();
    let cfg = test_config(&temp);
    let db = Database::open(&cfg).unwrap();
    db.migrate().unwrap();
    scan_fixtures(&cfg, &db).unwrap();

    let report = dry_run_report(&db, "anysearch", 1).unwrap();
    assert!(!report.will_execute);
    assert_eq!(report.skill_id, "anysearch");
    assert!(
        report
            .missing_env
            .contains(&"ANYSEARCH_API_KEY".to_string())
    );

    let err = print_run(&db, "anysearch", 1, false).unwrap_err();
    assert!(err.to_string().contains("only supports --dry-run"));
}

#[test]
fn safe_file_path_blocks_secret_and_traversal_reads() {
    let root = std::env::current_dir()
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join("skills")
        .join("anysearch");

    assert!(safe_skill_file_path(&root, "SKILL.md").is_ok());
    assert!(safe_skill_file_path(&root, ".env.example").is_ok());
    assert!(safe_skill_file_path(&root, ".env").is_err());
    assert!(safe_skill_file_path(&root, "../writing-helper/SKILL.md").is_err());
}
