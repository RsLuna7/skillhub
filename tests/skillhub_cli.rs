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

#[test]
fn demo_runs_sandboxed_tour() {
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_skillhub"))
        .arg("demo")
        .env("SKILLHUB_DATA_DIR", temp.path().join("data"))
        .env("SKILLHUB_INSTALL_DIR", temp.path().join("skills"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("indexed 2 skills"));
    assert!(stdout.contains("sketchy-cleaner: fail"));
    assert!(stdout.contains("hello-notes: pass"));
    assert!(stdout.contains("sketchy-cleaner: blocked"));
    assert!(stdout.contains("visible skills: hello-notes"));
    // The demo must not create the real data dir passed via env.
    assert!(!temp.path().join("data").exists());
}

#[test]
fn scan_accepts_force_flag() {
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_skillhub"))
        .arg("scan")
        .arg("--force")
        .env("USERPROFILE", temp.path())
        .env("SKILLHUB_DATA_DIR", temp.path().join("data"))
        .env("SKILLHUB_INSTALL_DIR", temp.path().join("skills"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Scanned"));
}

#[test]
fn ui_help_exposes_local_server_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_skillhub"))
        .arg("ui")
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--no-open"));
}
