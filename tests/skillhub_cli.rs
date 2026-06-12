use std::process::Command;

#[test]
fn setup_creates_isolated_config_and_prints_mcp_config() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    let install_dir = temp.path().join("skills");
    let output = Command::new(env!("CARGO_BIN_EXE_skillhub"))
        .arg("setup")
        .arg("--print-agent-instructions")
        .env("SKILLHUB_DATA_DIR", &data_dir)
        .env("SKILLHUB_INSTALL_DIR", &install_dir)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(data_dir.join("config.toml").exists());
    assert!(data_dir.join("index.sqlite").exists());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SkillHub setup complete"));
    assert!(stdout.contains("\"mcpServers\""));
    assert!(stdout.contains("first call skillhub.search_skills"));
}
