use skillhub::connect::connect;

#[test]
fn connect_codex_creates_config_and_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    let report = connect(home, "codex", "skillhub", false).unwrap();
    assert!(report.changed);
    assert!(report.backup_path.is_none());
    let content =
        std::fs::read_to_string(home.join(".codex").join("config.toml")).unwrap();
    assert!(content.contains("[mcp_servers.skillhub]"));
    assert!(content.contains("command = \"skillhub\""));

    let second = connect(home, "codex", "skillhub", false).unwrap();
    assert!(!second.changed);
}

#[test]
fn connect_codex_preserves_existing_config_and_backs_up() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    std::fs::write(home.join(".codex").join("config.toml"), "model = \"o4\"\n").unwrap();

    let report = connect(home, "codex", "skillhub", false).unwrap();
    assert!(report.changed);
    let backup = report.backup_path.unwrap();
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "model = \"o4\"\n");
    let content =
        std::fs::read_to_string(home.join(".codex").join("config.toml")).unwrap();
    assert!(content.starts_with("model = \"o4\"\n"));
    assert!(content.contains("[mcp_servers.skillhub]"));
}

#[test]
fn connect_dry_run_writes_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    let report = connect(home, "codex", "skillhub", true).unwrap();
    assert!(report.changed);
    assert!(!home.join(".codex").join("config.toml").exists());
}

#[test]
fn connect_rejects_unknown_agent() {
    let temp = tempfile::tempdir().unwrap();
    assert!(connect(temp.path(), "vscode", "skillhub", false).is_err());
}
