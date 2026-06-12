use crate::config::AppConfig;
use crate::db::Database;
use crate::trust::TrustStatus;
use crate::{audit, scan, trust};
use anyhow::Result;
use askama::Template;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

const UI_CSS: &str = include_str!("../assets/ui.css");

#[derive(Debug, Clone)]
pub struct UiState {
    pub cfg: AppConfig,
    pub db_path: PathBuf,
    /// Per-process synchronizer token. Embedded as a hidden field in every
    /// state-changing form and required on the matching POST, so a page on
    /// another origin cannot forge scan/audit/trust requests (it cannot read
    /// this token across the same-origin boundary).
    pub csrf_token: String,
}

pub fn run_ui(cfg: AppConfig, port: u16, open: bool) -> Result<()> {
    let url = format!("http://127.0.0.1:{port}");
    if open {
        let _ = webbrowser::open(&url);
    }
    println!("SkillHub UI: {url}");
    let state = UiState {
        db_path: cfg.index_path(),
        csrf_token: generate_csrf_token(),
        cfg,
    };
    // Migrate once at startup; request handlers just open a connection.
    Database::open(&state.cfg)?.migrate()?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        axum::serve(listener, router(state)).await?;
        Ok::<(), anyhow::Error>(())
    })
}

pub fn router(state: UiState) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/skills", get(skills_list))
        .route("/skills/{skill_id}", get(skill_detail))
        .route("/scan", post(scan_incremental))
        .route("/scan/force", post(scan_force))
        .route("/skills/{skill_id}/audit", post(audit_one))
        .route("/skills/{skill_id}/trust/allow", post(trust_allow))
        .route("/skills/{skill_id}/trust/block", post(trust_block))
        .route("/skills/{skill_id}/trust/reset", post(trust_reset))
        .route("/assets/ui.css", get(css))
        .with_state(state)
}

/// 32 random bytes from SQLite's PRNG, hex-encoded. No extra dependency; falls
/// back to a timestamp only if an in-memory connection cannot be opened.
fn generate_csrf_token() -> String {
    rusqlite::Connection::open_in_memory()
        .and_then(|conn| {
            conn.query_row("SELECT lower(hex(randomblob(32)))", [], |row| {
                row.get::<_, String>(0)
            })
        })
        .unwrap_or_else(|_| {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            format!("{nanos:032x}")
        })
}

/// An error rendered as a friendly HTML page. Internal causes are logged to the
/// terminal, never sent to the browser, so 500s do not leak implementation
/// detail. Explicit, safe messages (404, CSRF) are shown verbatim.
pub struct UiError {
    status: StatusCode,
    message: String,
}

#[derive(Template)]
#[template(path = "ui/error.html")]
struct ErrorTemplate {
    status: u16,
    message: String,
}

impl IntoResponse for UiError {
    fn into_response(self) -> Response {
        let body = ErrorTemplate {
            status: self.status.as_u16(),
            message: self.message.clone(),
        }
        .render()
        .unwrap_or_else(|_| format!("<h1>{}</h1><p>{}</p>", self.status, self.message));
        (self.status, Html(body)).into_response()
    }
}

/// Map any internal error to a generic 500 and log the real cause to stderr.
fn internal(error: impl std::fmt::Display) -> UiError {
    eprintln!("skillhub ui: {error}");
    UiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "Something went wrong. Check the terminal running `skillhub ui` for details."
            .to_string(),
    }
}

fn not_found(message: String) -> UiError {
    UiError {
        status: StatusCode::NOT_FOUND,
        message,
    }
}

/// Verify the synchronizer token from a urlencoded POST body.
fn verify_csrf(state: &UiState, body: &str) -> Result<(), UiError> {
    let provided = form_field(body, "csrf").unwrap_or_default();
    if !provided.is_empty() && provided == state.csrf_token {
        Ok(())
    } else {
        Err(UiError {
            status: StatusCode::FORBIDDEN,
            message: "Invalid or missing CSRF token. Reload the page and try again.".to_string(),
        })
    }
}

/// Read one field from an `application/x-www-form-urlencoded` body. CSRF tokens
/// are hex, so no percent-decoding is required.
fn form_field(body: &str, key: &str) -> Option<String> {
    body.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

#[derive(Debug, Clone)]
pub struct CountView {
    label: String,
    count: usize,
}

#[derive(Template)]
#[template(path = "ui/dashboard.html")]
struct DashboardTemplate {
    csrf: String,
    notice: Option<String>,
    total_skills: usize,
    source_counts: Vec<CountView>,
    risk_counts: Vec<CountView>,
    trust_counts: Vec<CountView>,
}

#[derive(Debug, Deserialize, Default)]
struct DashboardParams {
    notice: Option<String>,
}

#[derive(Debug, Clone)]
struct SkillRow {
    id: String,
    name: String,
    summary: String,
    risk: String,
    source_agent: String,
    trust: String,
    visibility: String,
}

#[derive(Debug, Deserialize, Default)]
struct SkillFilters {
    q: Option<String>,
    source: Option<String>,
    risk: Option<String>,
    trust: Option<String>,
}

#[derive(Template)]
#[template(path = "ui/skills.html")]
struct SkillsTemplate {
    filters: SkillFilters,
    skills: Vec<SkillRow>,
}

#[derive(Debug, Clone)]
struct SourceView {
    agent: String,
    root: String,
    priority: u8,
}

#[derive(Template)]
#[template(path = "ui/skill_detail.html")]
struct SkillDetailTemplate {
    csrf: String,
    summary: crate::skill::SkillUsageSummary,
    sources: Vec<SourceView>,
}

async fn dashboard(
    State(state): State<UiState>,
    Query(params): Query<DashboardParams>,
) -> Result<Html<String>, UiError> {
    let db = open_db(&state).map_err(internal)?;
    let skills = db.list_skills().map_err(internal)?;
    let mut source_counts = BTreeMap::<String, usize>::new();
    let mut risk_counts = BTreeMap::<String, usize>::new();
    let mut trust_counts = BTreeMap::<String, usize>::new();

    for skill in &skills {
        let source = display_source(&skill.source_agent);
        *source_counts.entry(source).or_default() += 1;
        *risk_counts
            .entry(skill.risk_level.as_str().to_string())
            .or_default() += 1;
        let trust = db
            .get_trust(&skill.id)
            .map_err(internal)?
            .map(|record| record.status)
            .unwrap_or(TrustStatus::Untrusted);
        *trust_counts.entry(trust.as_str().to_string()).or_default() += 1;
    }

    render(DashboardTemplate {
        csrf: state.csrf_token.clone(),
        notice: params.notice,
        total_skills: skills.len(),
        source_counts: count_views(source_counts),
        risk_counts: count_views(risk_counts),
        trust_counts: count_views(trust_counts),
    })
}

async fn skills_list(
    State(state): State<UiState>,
    Query(filters): Query<SkillFilters>,
) -> Result<Html<String>, UiError> {
    let db = open_db(&state).map_err(internal)?;
    // Text search reuses the same FTS+LIKE engine as the CLI and MCP, so the UI
    // returns identical matches; the other facets filter the result in memory.
    let base = match filters
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
    {
        Some(query) => db.search_skills(query).map_err(internal)?,
        None => db.list_skills().map_err(internal)?,
    };
    let mut rows = Vec::new();
    for skill in base {
        let trust_status = trust_status_for(&db, &skill.id)?;
        let row = SkillRow {
            id: skill.id.clone(),
            name: skill.name.clone(),
            summary: skill.summary.clone(),
            risk: skill.risk_level.as_str().to_string(),
            source_agent: display_source(&skill.source_agent),
            visibility: trust::visibility_for(&trust_status).as_str().to_string(),
            trust: trust_status.as_str().to_string(),
        };
        if matches_facets(&row, &filters) {
            rows.push(row);
        }
    }
    render(SkillsTemplate {
        filters,
        skills: rows,
    })
}

async fn skill_detail(
    State(state): State<UiState>,
    Path(skill_id): Path<String>,
) -> Result<Html<String>, UiError> {
    let db = open_db(&state).map_err(internal)?;
    if db.get_skill(&skill_id).map_err(internal)?.is_none() {
        return Err(not_found(format!("Skill not found: {skill_id}")));
    }
    let summary = crate::search::usage_summary(&db, &skill_id).map_err(internal)?;
    let sources = db
        .get_skill_sources(&skill_id)
        .map_err(internal)?
        .into_iter()
        .map(|(agent, root, priority)| SourceView {
            agent,
            root,
            priority,
        })
        .collect();
    render(SkillDetailTemplate {
        csrf: state.csrf_token.clone(),
        summary,
        sources,
    })
}

async fn scan_incremental(State(state): State<UiState>, body: String) -> Result<Redirect, UiError> {
    verify_csrf(&state, &body)?;
    let report = run_scan(&state, false)?;
    Ok(Redirect::to(&scan_notice_url(&report)))
}

async fn scan_force(State(state): State<UiState>, body: String) -> Result<Redirect, UiError> {
    verify_csrf(&state, &body)?;
    let report = run_scan(&state, true)?;
    Ok(Redirect::to(&scan_notice_url(&report)))
}

async fn audit_one(
    State(state): State<UiState>,
    Path(skill_id): Path<String>,
    body: String,
) -> Result<Redirect, UiError> {
    verify_csrf(&state, &body)?;
    let db = open_db(&state).map_err(internal)?;
    audit::audit_skill(&db, &skill_id).map_err(internal)?;
    Ok(Redirect::to(&format!("/skills/{skill_id}")))
}

async fn trust_allow(
    State(state): State<UiState>,
    Path(skill_id): Path<String>,
    body: String,
) -> Result<Redirect, UiError> {
    verify_csrf(&state, &body)?;
    let db = open_db(&state).map_err(internal)?;
    trust::allow(&db, &skill_id).map_err(internal)?;
    Ok(Redirect::to(&format!("/skills/{skill_id}")))
}

async fn trust_block(
    State(state): State<UiState>,
    Path(skill_id): Path<String>,
    body: String,
) -> Result<Redirect, UiError> {
    verify_csrf(&state, &body)?;
    let db = open_db(&state).map_err(internal)?;
    trust::block(&db, &skill_id, Some("blocked from SkillHub UI".to_string())).map_err(internal)?;
    Ok(Redirect::to(&format!("/skills/{skill_id}")))
}

async fn trust_reset(
    State(state): State<UiState>,
    Path(skill_id): Path<String>,
    body: String,
) -> Result<Redirect, UiError> {
    verify_csrf(&state, &body)?;
    let db = open_db(&state).map_err(internal)?;
    trust::reset(&db, &skill_id).map_err(internal)?;
    Ok(Redirect::to(&format!("/skills/{skill_id}")))
}

async fn css() -> Response {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], UI_CSS).into_response()
}

fn open_db(state: &UiState) -> Result<Database> {
    // Schema is migrated once at startup (run_ui); per-request we only open.
    Database::open(&state.cfg)
}

fn run_scan(state: &UiState, force: bool) -> Result<scan::ScanReport, UiError> {
    let db = open_db(state).map_err(internal)?;
    let home = crate::config::home_dir();
    let cwd = std::env::current_dir().unwrap_or_else(|_| home.clone());
    let roots = state.cfg.discovered_roots(&home, &cwd);
    scan::scan_roots(&state.cfg, &db, &roots, force).map_err(internal)
}

fn scan_notice_url(report: &scan::ScanReport) -> String {
    format!(
        "/?notice=Scanned%20{}%20roots%2C%20indexed%20{}%20skills",
        report.roots_scanned, report.skills_indexed
    )
}

fn count_views(counts: BTreeMap<String, usize>) -> Vec<CountView> {
    counts
        .into_iter()
        .map(|(label, count)| CountView { label, count })
        .collect()
}

fn trust_status_for(db: &Database, skill_id: &str) -> Result<TrustStatus, UiError> {
    db.get_trust(skill_id)
        .map_err(internal)
        .map(|record| record.map(|r| r.status).unwrap_or(TrustStatus::Untrusted))
}

fn display_source(source_agent: &str) -> String {
    if source_agent.is_empty() {
        "unknown".to_string()
    } else {
        source_agent.to_string()
    }
}

/// Filter by the non-text facets (text search is handled by the query engine).
fn matches_facets(row: &SkillRow, filters: &SkillFilters) -> bool {
    if let Some(source) = filters.source.as_deref().filter(|s| !s.trim().is_empty())
        && row.source_agent != source
    {
        return false;
    }
    if let Some(risk) = filters.risk.as_deref().filter(|r| !r.trim().is_empty())
        && row.risk != risk
    {
        return false;
    }
    if let Some(trust) = filters.trust.as_deref().filter(|t| !t.trim().is_empty())
        && row.trust != trust
    {
        return false;
    }
    true
}

fn render<T: Template>(template: T) -> Result<Html<String>, UiError> {
    template.render().map(Html).map_err(internal)
}
