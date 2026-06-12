use skillhub::providers::{ProviderRegistry, priority};
use std::path::Path;

#[test]
fn claude_provider_includes_user_global_and_plugin_roots() {
    let home = Path::new("/home/u");
    let cwd = Path::new("/home/u/project");
    let roots = ProviderRegistry::with_builtins().all_roots(home, cwd, &[]);

    assert!(roots.iter().any(|r| r.agent == "claude"
        && r.path.ends_with(".claude/skills")
        && r.priority == priority::USER_GLOBAL));
    assert!(roots.iter().any(|r| r.agent == "claude"
        && r.priority == priority::PLUGIN
        && r.path.to_string_lossy().contains("plugins")));
}

#[test]
fn project_provider_walks_up_for_dot_skills() {
    let home = Path::new("/home/u");
    let cwd = Path::new("/home/u/project/sub");
    let roots = ProviderRegistry::with_builtins().all_roots(home, cwd, &[]);
    assert!(roots.iter().any(|r| r.agent == "project"
        && r.path.ends_with(".skills")
        && r.priority == priority::PROJECT));
}

#[test]
fn user_config_roots_are_highest_priority() {
    let home = Path::new("/home/u");
    let cwd = Path::new("/home/u/project");
    let extra = vec![std::path::PathBuf::from("/opt/custom/skills")];
    let roots = ProviderRegistry::with_builtins().all_roots(home, cwd, &extra);
    let custom = roots
        .iter()
        .find(|r| r.path == Path::new("/opt/custom/skills"))
        .expect("custom root present");
    assert_eq!(custom.agent, "user-config");
    assert_eq!(custom.priority, priority::USER_CONFIG);
}

#[test]
fn roots_are_deduplicated_by_path_keeping_highest_priority() {
    let home = Path::new("/home/u");
    let cwd = Path::new("/home/u/project");
    let extra = vec![home.join(".claude/skills")];
    let roots = ProviderRegistry::with_builtins().all_roots(home, cwd, &extra);
    let matches: Vec<_> = roots
        .iter()
        .filter(|r| r.path == home.join(".claude/skills"))
        .collect();
    assert_eq!(matches.len(), 1, "duplicate path must collapse to one root");
    assert_eq!(matches[0].priority, priority::USER_CONFIG);
}

#[test]
fn config_discovered_roots_combine_providers_and_user_roots() {
    use skillhub::config::AppConfig;
    let cfg = AppConfig {
        data_dir: "/tmp/data".into(),
        default_install_dir: "/tmp/install".into(),
        scan_roots: vec!["/opt/custom/skills".into()],
        mcp: skillhub::config::McpConfig {
            max_file_chars: 12000,
        },
        config_path: std::path::PathBuf::from("/tmp/config.toml"),
    };
    let home = Path::new("/home/u");
    let cwd = Path::new("/home/u/project");
    let roots = cfg.discovered_roots(home, cwd);
    assert!(
        roots
            .iter()
            .any(|r| r.agent == "user-config" && r.path == Path::new("/opt/custom/skills"))
    );
    assert!(roots.iter().any(|r| r.agent == "claude"));
}
