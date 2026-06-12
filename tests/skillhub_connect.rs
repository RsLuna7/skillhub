use skillhub::connect::connect;

#[test]
fn connect_codex_creates_config_and_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    let report = connect(home, "codex", "skillhub", false).unwrap();
    assert!(report.changed);
    assert!(report.backup_path.is_none());
    let content = std::fs::read_to_string(home.join(".codex").join("config.toml")).unwrap();
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
    assert_eq!(
        std::fs::read_to_string(&backup).unwrap(),
        "model = \"o4\"\n"
    );
    let content = std::fs::read_to_string(home.join(".codex").join("config.toml")).unwrap();
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

#[test]
fn connect_claude_merges_json_preserving_existing_keys() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    std::fs::write(
        home.join(".claude.json"),
        r#"{"theme":"dark","mcpServers":{"other":{"command":"other"}}}"#,
    )
    .unwrap();

    let report = connect(home, "claude", "skillhub", false).unwrap();
    assert!(report.changed);
    assert!(report.backup_path.is_some());

    let content = std::fs::read_to_string(home.join(".claude.json")).unwrap();
    let root: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(root["theme"], "dark");
    assert_eq!(root["mcpServers"]["other"]["command"], "other");
    assert_eq!(root["mcpServers"]["skillhub"]["command"], "skillhub");
    assert_eq!(root["mcpServers"]["skillhub"]["args"][0], "mcp");

    let second = connect(home, "claude", "skillhub", false).unwrap();
    assert!(!second.changed);
}

#[test]
fn connect_cursor_creates_config_when_missing() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    let report = connect(home, "cursor", "skillhub", false).unwrap();
    assert!(report.changed);
    assert!(report.backup_path.is_none());

    let content = std::fs::read_to_string(home.join(".cursor").join("mcp.json")).unwrap();
    let root: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(root["mcpServers"]["skillhub"]["command"], "skillhub");
}

#[test]
fn connect_claude_rejects_malformed_json() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    std::fs::write(home.join(".claude.json"), "not json at all").unwrap();
    assert!(connect(home, "claude", "skillhub", false).is_err());
    // Original file must be untouched after a failed connect.
    assert_eq!(
        std::fs::read_to_string(home.join(".claude.json")).unwrap(),
        "not json at all"
    );
}
