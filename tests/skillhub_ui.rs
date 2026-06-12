use axum::http::StatusCode;
use skillhub::config::{AppConfig, McpConfig};
use skillhub::db::Database;
use skillhub::scan::scan_roots;
use skillhub::trust;
use skillhub::ui::{UiState, router};
use std::fs;
use std::io::{Read, Write};

fn test_config(temp: &tempfile::TempDir) -> AppConfig {
    AppConfig {
        data_dir: temp.path().join("data").to_string_lossy().to_string(),
        default_install_dir: temp.path().join("install").to_string_lossy().to_string(),
        scan_roots: Vec::new(),
        mcp: McpConfig {
            max_file_chars: 12_000,
        },
        config_path: temp.path().join("config.toml"),
    }
}

fn test_state(temp: &tempfile::TempDir) -> UiState {
    let cfg = test_config(temp);
    let db = Database::open(&cfg).unwrap();
    db.migrate().unwrap();
    UiState {
        db_path: cfg.index_path(),
        cfg,
    }
}

fn write_skill(root: &std::path::Path, id: &str, name: &str, body: &str) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {body}\n---\n# {name}\n\n{body}\n"),
    )
    .unwrap();
}

fn seed_two_skills(temp: &tempfile::TempDir) -> UiState {
    let cfg = test_config(temp);
    let db = Database::open(&cfg).unwrap();
    db.migrate().unwrap();
    let root = temp.path().join("skills");
    write_skill(&root, "safe-one", "Safe One", "write notes");
    write_skill(&root, "risky-one", "Risky One", "delete caches with shell");
    let roots = vec![skillhub::providers::DiscoveredRoot {
        agent: "generic".into(),
        path: root,
        priority: skillhub::providers::priority::USER_GLOBAL,
    }];
    scan_roots(&cfg, &db, &roots, true).unwrap();
    trust::block(&db, "risky-one", Some("review pending".into())).unwrap();
    UiState {
        db_path: cfg.index_path(),
        cfg,
    }
}

#[tokio::test]
async fn ui_dashboard_route_returns_ok() {
    let temp = tempfile::tempdir().unwrap();

    let (status, text) = get_text(test_state(&temp), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("SkillHub"));
}

#[tokio::test]
async fn ui_css_route_returns_stylesheet() {
    let temp = tempfile::tempdir().unwrap();

    let (status, text) = get_text(test_state(&temp), "/assets/ui.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains(".shell"));
}

#[tokio::test]
async fn dashboard_summarizes_skills_source_risk_and_trust() {
    let temp = tempfile::tempdir().unwrap();

    let (status, text) = get_text(seed_two_skills(&temp), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("2 skills"));
    assert!(text.contains("generic"));
    assert!(text.contains("high"));
    assert!(text.contains("blocked"));
}

async fn request(state: UiState, method: &str, uri: &str) -> (StatusCode, String) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, router(state)).await.unwrap();
    });

    let method = method.to_string();
    let uri = uri.to_string();
    let response = tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(addr).unwrap();
        let request = format!(
            "{method} {uri} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    })
    .await
    .unwrap();
    server.abort();
    let _ = server.await;

    let response = String::from_utf8_lossy(&response).to_string();
    let status = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .and_then(|code| StatusCode::from_u16(code).ok())
        .unwrap();
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    (status, body)
}

async fn get_text(state: UiState, uri: &str) -> (StatusCode, String) {
    request(state, "GET", uri).await
}

async fn post(state: UiState, uri: &str) -> StatusCode {
    request(state, "POST", uri).await.0
}

#[tokio::test]
async fn skills_list_shows_and_filters_skills() {
    let temp = tempfile::tempdir().unwrap();

    let (status, text) = get_text(seed_two_skills(&temp), "/skills").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("Safe One"));
    assert!(text.contains("Risky One"));

    let (status, text) = get_text(seed_two_skills(&temp), "/skills?q=safe").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("Safe One"));
    assert!(!text.contains("Risky One"));

    let (status, text) = get_text(seed_two_skills(&temp), "/skills?risk=high").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("Risky One"));
    assert!(!text.contains("Safe One"));

    let (status, text) = get_text(seed_two_skills(&temp), "/skills?trust=blocked").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("Risky One"));
    assert!(text.contains("blocked"));
    assert!(!text.contains("Safe One"));
}

#[tokio::test]
async fn skill_detail_shows_metadata_and_unknown_is_404() {
    let temp = tempfile::tempdir().unwrap();

    let (status, text) = get_text(seed_two_skills(&temp), "/skills/risky-one").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("Risky One"));
    assert!(text.contains("generic"));
    assert!(text.contains("high"));
    assert!(text.contains("blocked"));
    assert!(text.contains("SKILL.md"));
    assert!(text.contains("action=\"/skills/risky-one/audit\""));
    assert!(text.contains("action=\"/skills/risky-one/trust/allow\""));
    assert!(text.contains("action=\"/skills/risky-one/trust/block\""));
    assert!(text.contains("action=\"/skills/risky-one/trust/reset\""));

    let (status, _text) = get_text(seed_two_skills(&temp), "/skills/missing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn scan_actions_redirect_and_update_index() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("skills");
    write_skill(&root, "new-one", "New One", "fresh skill");
    let mut cfg = test_config(&temp);
    cfg.scan_roots = vec![root.to_string_lossy().to_string()];
    let db = Database::open(&cfg).unwrap();
    db.migrate().unwrap();
    let state = UiState {
        db_path: cfg.index_path(),
        cfg: cfg.clone(),
    };

    let status = post(state.clone(), "/scan").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(
        Database::open(&cfg)
            .unwrap()
            .get_skill("new-one")
            .unwrap()
            .is_some()
    );

    let status = post(state, "/scan/force").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn dashboard_can_show_scan_notice() {
    let temp = tempfile::tempdir().unwrap();
    let (status, text) = get_text(test_state(&temp), "/?notice=Scanned%201%20root").await;
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains("Scanned 1 root"));
}

#[tokio::test]
async fn audit_and_trust_actions_redirect_and_update_state() {
    let temp = tempfile::tempdir().unwrap();
    let state = seed_two_skills(&temp);
    let cfg = state.cfg.clone();

    let status = post(state.clone(), "/skills/safe-one/audit").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(
        Database::open(&cfg)
            .unwrap()
            .get_audit("safe-one")
            .unwrap()
            .is_some()
    );

    let status = post(state.clone(), "/skills/safe-one/trust/allow").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        trust::trust_status(&Database::open(&cfg).unwrap(), "safe-one").unwrap(),
        trust::TrustStatus::Trusted
    );

    let status = post(state.clone(), "/skills/safe-one/trust/block").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        trust::trust_status(&Database::open(&cfg).unwrap(), "safe-one").unwrap(),
        trust::TrustStatus::Blocked
    );

    let status = post(state, "/skills/safe-one/trust/reset").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        trust::trust_status(&Database::open(&cfg).unwrap(), "safe-one").unwrap(),
        trust::TrustStatus::Untrusted
    );
}
