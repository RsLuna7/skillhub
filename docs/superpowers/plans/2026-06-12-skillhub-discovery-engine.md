# SkillHub v0.5 Discovery Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace SkillHub's hardcoded, home-relative, snapshot-only scan roots with a per-OS provider registry that records skill provenance, deterministically de-duplicates by priority, and keeps the index fresh via lazy incremental re-scan — all with no resident daemon.

**Architecture:** A new `providers` module turns the hardcoded root list into one `SkillProvider` per agent (Claude/Cursor/Codex/generic/project), each expanding its skill directories per-OS and tagging them with an agent name and priority. The scan engine consumes `(agent, root, priority)` tuples instead of bare paths, parses skills with a tightened typed-manifest detector, collects candidates per id, and writes the highest-priority winner to `skills` plus every location to a new `skill_sources` table. A `scan_state` table stores a cheap per-root signature so `scan` skips unchanged roots and MCP `list`/`search` re-scan only stale roots before returning.

**Tech Stack:** Rust (edition 2024), rusqlite (bundled SQLite), walkdir, serde/serde_json, regex, tempfile — all already in `Cargo.toml`. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-06-12-skillhub-discovery-engine-design.md`

**Deliberate spec deviation:** the spec says "real YAML parsing into a typed `SkillManifest`". Because the only YAML used is simple `key: value` frontmatter and the project's v0.4 rule is "no new dependencies" (no maintained pure-Rust YAML crate is in-tree), this plan implements a dep-free typed `SkillManifest` line parser. It satisfies the spec's *intent* (typed, sturdier than `str::find("---")`, surfaces malformed frontmatter as an error) without adding an unmaintained crate.

---

## File structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/providers.rs` | Create | `DiscoveredRoot`, `SkillProvider` trait, built-in per-OS providers, `ProviderRegistry` |
| `src/manifest.rs` | Create | `SkillManifest` typed frontmatter parser (dep-free), errors on malformed input |
| `src/lib.rs` | Modify | Register `pub mod manifest;` and `pub mod providers;` |
| `src/config.rs` | Modify | `scan_roots` becomes user-supplied *extra* roots; provider registry supplies defaults; new `discovered_roots()` |
| `src/skill.rs` | Modify | Add `source_agent: String`, `source_root: String` to `Skill` |
| `src/db.rs` | Modify | Migration: `source_agent`/`source_root` columns, `skill_sources`, `scan_state`; new methods |
| `src/scan.rs` | Modify | Consume `DiscoveredRoot`s, tightened detection via `manifest`, priority dedup, incremental skip, write provenance |
| `src/mcp.rs` | Modify | Lazy freshness check before list/search; include `source_agent` in JSON |
| `tests/skillhub_providers.rs` | Create | Per-provider path tests, cross-OS via injected home/cwd |
| `tests/skillhub_discovery.rs` | Create | Dedup, provenance, incremental-skip, freshness integration tests |
| `tests/skillhub_core.rs` | Modify | Fixture-count + `Skill` construction updates |
| `tests/skillhub_control_plane.rs` | Modify | `source_agent` present in MCP output |
| `Cargo.toml`, `CHANGELOG.md`, `ROADMAP.md`, `README.md`, `README.zh-CN.md` | Modify | 0.5.0, changelog, roadmap, docs (EN+CN) |

**Verification gates (run after every task, all must pass):**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Tasks 2 and 6 add functions wired into production only in Tasks 3/7. For those intermediate tasks, gate with `cargo test --lib` (or add `#[allow(dead_code)]` and remove it when wiring). Pick one and be consistent.

---

### Task 0: Branch and commit this plan

**Files:**
- Create branch `feat/v0.5-discovery` from `feat/v0.4-adoption`
- Commit the spec + this plan

- [ ] **Step 1: Branch from the v0.4 baseline**

```bash
git checkout feat/v0.4-adoption
git checkout -b feat/v0.5-discovery
```

- [ ] **Step 2: Verify the baseline is green**

Run: `cargo test`
Expected: all v0.4 tests pass (30 tests). If not, STOP and fix the baseline first.

- [ ] **Step 3: Commit the design docs (if not already committed)**

```bash
git add docs/superpowers/specs/2026-06-12-skillhub-discovery-engine-design.md docs/superpowers/plans/2026-06-12-skillhub-discovery-engine.md
git commit -m "docs: v0.5 discovery engine spec + plan"
```

---

### Task 1: Verify real plugin/Cursor/Codex skill paths (investigation)

The spec flags one open item: **do not assume** where Claude Code plugin skills and Cursor skills live. Verify against a real install before coding `providers.rs`.

**Files:**
- Create: `docs/superpowers/notes/2026-06-12-agent-skill-paths.md` (findings)

- [ ] **Step 1: Locate each agent's skill directories on this machine**

Run (Windows PowerShell; adapt for the host OS):

```powershell
# Claude Code user skills
Get-ChildItem "$env:USERPROFILE\.claude\skills" -Directory -ErrorAction SilentlyContinue | Select-Object FullName
# Claude Code plugin skills (cache layout observed in this repo's session)
Get-ChildItem "$env:USERPROFILE\.claude\plugins" -Recurse -Filter SKILL.md -ErrorAction SilentlyContinue | Select-Object -First 5 FullName
# Codex
Get-ChildItem "$env:USERPROFILE\.codex\skills" -Directory -ErrorAction SilentlyContinue | Select-Object FullName
# Cursor (probe likely locations)
Get-ChildItem "$env:USERPROFILE\.cursor" -Recurse -Filter SKILL.md -ErrorAction SilentlyContinue | Select-Object -First 5 FullName
```

- [ ] **Step 2: Record the confirmed glob patterns**

Write `docs/superpowers/notes/2026-06-12-agent-skill-paths.md` with, for each agent, the **confirmed** directory pattern(s) and the OS it was observed on. Mark any that could not be confirmed as "unconfirmed — registry will include it but it may match nothing." This file is the source of truth Task 2 implements against.

Note: Claude Code plugin skills observed in this session live under
`~/.claude/plugins/cache/<marketplace>/<plugin>/<version>/skills/<skill>/SKILL.md`.
Record the exact depth so the registry's glob (or fixed join + walk) is correct.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/notes/2026-06-12-agent-skill-paths.md
git commit -m "docs: confirmed agent skill directory paths"
```

---

### Task 2: `providers` module — DiscoveredRoot, trait, built-in providers, registry

**Files:**
- Create: `src/providers.rs`
- Modify: `src/lib.rs` (add `pub mod providers;` — keep module list alphabetical)
- Test: create `tests/skillhub_providers.rs`

Providers take `home` and `cwd` as parameters (never read the real environment directly) so tests are hermetic and cross-OS. Priority constants match the spec: `user-config 50 > project 40 > user-global 20 > plugin 10`.

- [ ] **Step 1: Write the failing tests**

Create `tests/skillhub_providers.rs`:

```rust
use skillhub::providers::{ProviderRegistry, priority};
use std::path::Path;

#[test]
fn claude_provider_includes_user_global_and_plugin_roots() {
    let home = Path::new("/home/u");
    let cwd = Path::new("/home/u/project");
    let roots = ProviderRegistry::with_builtins().all_roots(home, cwd, &[]);

    // user-global ~/.claude/skills present at priority 20
    assert!(roots.iter().any(|r| r.agent == "claude"
        && r.path.ends_with(".claude/skills")
        && r.priority == priority::USER_GLOBAL));
    // a plugin root present at priority 10
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
    // Pass ~/.claude/skills as an explicit extra root too; it must collapse to
    // a single entry at the higher (user-config) priority.
    let extra = vec![home.join(".claude/skills")];
    let roots = ProviderRegistry::with_builtins().all_roots(home, cwd, &extra);
    let matches: Vec<_> = roots
        .iter()
        .filter(|r| r.path == home.join(".claude/skills"))
        .collect();
    assert_eq!(matches.len(), 1, "duplicate path must collapse to one root");
    assert_eq!(matches[0].priority, priority::USER_CONFIG);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test skillhub_providers`
Expected: compile error — module `providers` not found.

- [ ] **Step 3: Implement `src/providers.rs`**

```rust
use std::path::{Path, PathBuf};

pub mod priority {
    pub const USER_CONFIG: u8 = 50;
    pub const PROJECT: u8 = 40;
    pub const USER_GLOBAL: u8 = 20;
    pub const PLUGIN: u8 = 10;
}

/// A directory to scan, tagged with the agent it belongs to and a priority used
/// to resolve duplicate skill ids (higher wins).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredRoot {
    pub agent: String,
    pub path: PathBuf,
    pub priority: u8,
}

impl DiscoveredRoot {
    fn new(agent: &str, path: PathBuf, priority: u8) -> Self {
        Self {
            agent: agent.to_string(),
            path,
            priority,
        }
    }
}

/// A provider knows which directories on *this* machine belong to one agent.
/// It performs pure path logic only — it never scans or parses.
pub trait SkillProvider {
    fn name(&self) -> &str;
    fn roots(&self, home: &Path, cwd: &Path) -> Vec<DiscoveredRoot>;
}

pub struct ClaudeProvider;
impl SkillProvider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }
    fn roots(&self, home: &Path, cwd: &Path) -> Vec<DiscoveredRoot> {
        let mut roots = vec![DiscoveredRoot::new(
            "claude",
            home.join(".claude").join("skills"),
            priority::USER_GLOBAL,
        )];
        // Plugin skills: ~/.claude/plugins/**/skills (see Task 1 notes for depth).
        for plugin_skills in plugin_skill_dirs(&home.join(".claude").join("plugins")) {
            roots.push(DiscoveredRoot::new("claude", plugin_skills, priority::PLUGIN));
        }
        // Project-level Claude skills.
        if let Some(dir) = find_up(cwd, &[".claude", "skills"]) {
            roots.push(DiscoveredRoot::new("claude", dir, priority::PROJECT));
        }
        roots
    }
}

pub struct CursorProvider;
impl SkillProvider for CursorProvider {
    fn name(&self) -> &str {
        "cursor"
    }
    fn roots(&self, home: &Path, _cwd: &Path) -> Vec<DiscoveredRoot> {
        vec![DiscoveredRoot::new(
            "cursor",
            home.join(".cursor").join("skills"),
            priority::USER_GLOBAL,
        )]
    }
}

pub struct CodexProvider;
impl SkillProvider for CodexProvider {
    fn name(&self) -> &str {
        "codex"
    }
    fn roots(&self, home: &Path, _cwd: &Path) -> Vec<DiscoveredRoot> {
        vec![DiscoveredRoot::new(
            "codex",
            home.join(".codex").join("skills"),
            priority::USER_GLOBAL,
        )]
    }
}

pub struct GenericProvider;
impl SkillProvider for GenericProvider {
    fn name(&self) -> &str {
        "generic"
    }
    fn roots(&self, home: &Path, _cwd: &Path) -> Vec<DiscoveredRoot> {
        let mut roots = vec![DiscoveredRoot::new(
            "generic",
            home.join(".agents").join("skills"),
            priority::USER_GLOBAL,
        )];
        // XDG-style ~/.config/*/skills
        let config = home.join(".config");
        if let Ok(entries) = std::fs::read_dir(&config) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let candidate = entry.path().join("skills");
                    roots.push(DiscoveredRoot::new(
                        "generic",
                        candidate,
                        priority::USER_GLOBAL,
                    ));
                }
            }
        }
        roots
    }
}

pub struct ProjectProvider;
impl SkillProvider for ProjectProvider {
    fn name(&self) -> &str {
        "project"
    }
    fn roots(&self, _home: &Path, cwd: &Path) -> Vec<DiscoveredRoot> {
        let mut roots = Vec::new();
        if let Some(dir) = find_up(cwd, &[".skills"]) {
            roots.push(DiscoveredRoot::new("project", dir, priority::PROJECT));
        }
        roots
    }
}

/// Walk from `start` up to the filesystem root, returning the first existing
/// `start/.../segments`. Returns the candidate even if it does not exist when
/// at `start` itself, so a fresh project still gets a project root entry.
fn find_up(start: &Path, segments: &[&str]) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let mut candidate = dir.to_path_buf();
        for seg in segments {
            candidate = candidate.join(seg);
        }
        if candidate.exists() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    // Fallback: the candidate rooted at `start` (may not exist yet).
    let mut candidate = start.to_path_buf();
    for seg in segments {
        candidate = candidate.join(seg);
    }
    Some(candidate)
}

/// Enumerate `<plugins_dir>/**/skills` directories. Bounded depth keeps this
/// cheap; the exact depth is confirmed in Task 1.
fn plugin_skill_dirs(plugins_dir: &Path) -> Vec<PathBuf> {
    use walkdir::WalkDir;
    if !plugins_dir.exists() {
        return Vec::new();
    }
    WalkDir::new(plugins_dir)
        .max_depth(6)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir() && entry.file_name() == "skills")
        .map(|entry| entry.into_path())
        .collect()
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn SkillProvider>>,
}

impl ProviderRegistry {
    pub fn with_builtins() -> Self {
        Self {
            providers: vec![
                Box::new(ClaudeProvider),
                Box::new(CursorProvider),
                Box::new(CodexProvider),
                Box::new(GenericProvider),
                Box::new(ProjectProvider),
            ],
        }
    }

    /// All roots from every provider plus user-configured extras, de-duplicated
    /// by canonical-ish path, keeping the highest priority on collision.
    pub fn all_roots(
        &self,
        home: &Path,
        cwd: &Path,
        extra: &[PathBuf],
    ) -> Vec<DiscoveredRoot> {
        let mut all: Vec<DiscoveredRoot> = Vec::new();
        for path in extra {
            all.push(DiscoveredRoot::new(
                "user-config",
                path.clone(),
                priority::USER_CONFIG,
            ));
        }
        for provider in &self.providers {
            all.extend(provider.roots(home, cwd));
        }
        dedupe_by_path(all)
    }
}

fn dedupe_by_path(roots: Vec<DiscoveredRoot>) -> Vec<DiscoveredRoot> {
    let mut out: Vec<DiscoveredRoot> = Vec::new();
    for root in roots {
        if let Some(existing) = out.iter_mut().find(|r| r.path == root.path) {
            if root.priority > existing.priority {
                existing.priority = root.priority;
                existing.agent = root.agent;
            }
        } else {
            out.push(root);
        }
    }
    out
}
```

And in `src/lib.rs` add `pub mod manifest;` and `pub mod providers;` in alphabetical position (e.g. after `pub mod install;` add `pub mod manifest;`, and `pub mod providers;` before `pub mod run;`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test skillhub_providers`
Expected: 4 tests PASS.

- [ ] **Step 5: Run gates and commit**

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add src/providers.rs src/lib.rs tests/skillhub_providers.rs
git commit -m "feat(providers): per-OS provider registry with priority roots"
```

(If clippy flags `manifest` as missing, temporarily declare an empty `src/manifest.rs` with `// placeholder` and `pub mod manifest;`; Task 6 fills it. Or add `manifest` to lib.rs only in Task 6 and skip it here.)

---

### Task 3: Wire the registry into config

`scan_roots` becomes *extra* user roots; defaults come from the registry. Backward compatible: an existing `config.toml` listing the old defaults still works (those paths become `user-config` roots, deduped against the same provider paths).

**Files:**
- Modify: `src/config.rs`
- Test: `tests/skillhub_providers.rs` (add a config-level test)

- [ ] **Step 1: Write the failing test**

Append to `tests/skillhub_providers.rs`:

```rust
#[test]
fn config_discovered_roots_combine_providers_and_user_roots() {
    use skillhub::config::AppConfig;
    let cfg = AppConfig {
        data_dir: "/tmp/data".into(),
        default_install_dir: "/tmp/install".into(),
        scan_roots: vec!["/opt/custom/skills".into()],
        mcp: skillhub::config::McpConfig { max_file_chars: 12000 },
        config_path: std::path::PathBuf::from("/tmp/config.toml"),
    };
    let home = Path::new("/home/u");
    let cwd = Path::new("/home/u/project");
    let roots = cfg.discovered_roots(home, cwd);
    assert!(roots.iter().any(|r| r.agent == "user-config"
        && r.path == Path::new("/opt/custom/skills")));
    assert!(roots.iter().any(|r| r.agent == "claude"));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test skillhub_providers config_discovered_roots`
Expected: compile error — no method `discovered_roots`.

- [ ] **Step 3: Implement `discovered_roots` and slim the default**

In `src/config.rs`:

(a) Change the `Default` impl's `scan_roots` to be **empty** by default (providers now supply the built-ins), keeping the field for user extras:

```rust
            scan_roots: Vec::new(),
```

(b) Add this method inside `impl AppConfig` (near `expanded_scan_roots`):

```rust
    /// Provider-supplied roots plus user `scan_roots` extras, deduped by path.
    pub fn discovered_roots(
        &self,
        home: &std::path::Path,
        cwd: &std::path::Path,
    ) -> Vec<crate::providers::DiscoveredRoot> {
        let extra = self.expanded_scan_roots();
        crate::providers::ProviderRegistry::with_builtins().all_roots(home, cwd, &extra)
    }
```

(Keep `expanded_scan_roots` — it now expands only user extras.)

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test --test skillhub_providers`
Expected: 5 tests PASS.

- [ ] **Step 5: Run gates and commit**

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add src/config.rs tests/skillhub_providers.rs
git commit -m "feat(config): discovered_roots from provider registry + user extras"
```

---

### Task 4: DB schema — provenance columns, skill_sources, scan_state

**Files:**
- Modify: `src/db.rs`
- Test: `tests/skillhub_discovery.rs` (create)

- [ ] **Step 1: Write the failing test**

Create `tests/skillhub_discovery.rs`:

```rust
use skillhub::config::{AppConfig, McpConfig};
use skillhub::db::Database;

fn temp_db(temp: &tempfile::TempDir) -> (AppConfig, Database) {
    let cfg = AppConfig {
        data_dir: temp.path().join("data").to_string_lossy().to_string(),
        default_install_dir: temp.path().join("install").to_string_lossy().to_string(),
        scan_roots: Vec::new(),
        mcp: McpConfig { max_file_chars: 12000 },
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
    assert_eq!(sources[1], ("project".to_string(), "/p/demo".to_string(), 40));
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
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test skillhub_discovery`
Expected: compile error — methods `replace_skill_sources`, `get_skill_sources`, `get_scan_signature`, `set_scan_signature` not found.

- [ ] **Step 3: Add migrations and methods**

In `src/db.rs` `migrate()`, after the `audit_results` table block (before the closing `"#`), add:

```sql
            CREATE TABLE IF NOT EXISTS skill_sources (
                skill_id TEXT NOT NULL,
                agent TEXT NOT NULL,
                root TEXT NOT NULL,
                priority INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (skill_id, root)
            );
            CREATE TABLE IF NOT EXISTS scan_state (
                root TEXT PRIMARY KEY,
                signature TEXT NOT NULL,
                last_scanned_at TEXT NOT NULL
            );
```

After the existing `ALTER TABLE skills ADD COLUMN detected_capabilities_json ...` line, add two more idempotent ALTERs (the `let _ =` pattern swallows "duplicate column" on re-run):

```rust
        let _ = self.conn.execute(
            "ALTER TABLE skills ADD COLUMN source_agent TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE skills ADD COLUMN source_root TEXT NOT NULL DEFAULT ''",
            [],
        );
```

Add these methods inside `impl Database` (after `record_audit`/`get_audit`):

```rust
    pub fn replace_skill_sources(
        &self,
        skill_id: &str,
        sources: &[(String, String, u8)],
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .execute("DELETE FROM skill_sources WHERE skill_id = ?1", [skill_id])?;
        for (agent, root, priority) in sources {
            self.conn.execute(
                "INSERT INTO skill_sources (skill_id, agent, root, priority, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![skill_id, agent, root, *priority as i64, now],
            )?;
        }
        Ok(())
    }

    pub fn get_skill_sources(&self, skill_id: &str) -> Result<Vec<(String, String, u8)>> {
        let mut stmt = self.conn.prepare(
            "SELECT agent, root, priority FROM skill_sources WHERE skill_id = ?1 ORDER BY priority DESC, root",
        )?;
        let rows = stmt.query_map([skill_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? as u8,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_scan_signature(&self, root: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT signature FROM scan_state WHERE root = ?1",
                [root],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn set_scan_signature(&self, root: &str, signature: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"
            INSERT INTO scan_state (root, signature, last_scanned_at) VALUES (?1, ?2, ?3)
            ON CONFLICT(root) DO UPDATE SET signature=excluded.signature, last_scanned_at=excluded.last_scanned_at
            "#,
            params![root, signature, now],
        )?;
        Ok(())
    }
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test --test skillhub_discovery`
Expected: 2 tests PASS.

- [ ] **Step 5: Run gates and commit**

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add src/db.rs tests/skillhub_discovery.rs
git commit -m "feat(db): skill_sources + scan_state tables and provenance columns"
```

---

### Task 5: `Skill` provenance fields + persistence

**Files:**
- Modify: `src/skill.rs`, `src/db.rs`
- Modify: `tests/skillhub_core.rs` (constructions that build `Skill` literals, if any)

- [ ] **Step 1: Add fields to `Skill`**

In `src/skill.rs`, add to `struct Skill` after `pub last_scanned_at: String,`:

```rust
    #[serde(default)]
    pub source_agent: String,
    #[serde(default)]
    pub source_root: String,
```

(`#[serde(default)]` keeps old JSON deserializable — additive.)

- [ ] **Step 2: Persist and read the fields**

In `src/db.rs` `upsert_skill`, extend the INSERT column list and values. Change the column list to add `source_agent, source_root` before `created_at`, bump the placeholder count to `?19`, and add to the `ON CONFLICT ... DO UPDATE SET` block:

```sql
                source_agent=excluded.source_agent,
                source_root=excluded.source_root,
```

In the `params![...]` add after `skill.last_scanned_at,`:

```rust
                skill.source_agent,
                skill.source_root,
```

(So the order is: ... `skill.last_scanned_at, skill.source_agent, skill.source_root, now, now`. Update the VALUES list to `... ?15, ?16, ?17, ?18, ?19)` where `?16`=source_agent, `?17`=source_root, `?18`/`?19`=created/updated.)

In `skill_from_row`, add before the closing `})`:

```rust
        source_agent: row.get("source_agent").unwrap_or_default(),
        source_root: row.get("source_root").unwrap_or_default(),
```

- [ ] **Step 3: Fix any `Skill { ... }` literals in tests**

Run: `cargo test 2>&1 | grep -n "missing field"` — if `tests/skillhub_core.rs` (or others) build a `Skill` literal, add `source_agent: String::new(), source_root: String::new(),`. If they only build via `scan`, no change is needed.

- [ ] **Step 4: Run gates and commit**

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add src/skill.rs src/db.rs tests/
git commit -m "feat(skill): persist source_agent/source_root provenance"
```

---

### Task 6: Tightened detection — typed `SkillManifest`

**Files:**
- Create: `src/manifest.rs`
- Modify: `src/lib.rs` (ensure `pub mod manifest;` is present)
- Test: unit tests inside `src/manifest.rs`

- [ ] **Step 1: Write the failing unit tests**

Create `src/manifest.rs`:

```rust
//! Dep-free typed parser for SKILL.md / skill.yaml frontmatter.
//! Handles the simple `key: value` frontmatter SkillHub actually uses; surfaces
//! malformed frontmatter as an error instead of silently missing a skill.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillManifest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ManifestError {
    NoFrontmatter,
    Unterminated,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_description() {
        let md = "---\nname: Hello\ndescription: \"Does a thing\"\n---\n# Hello\n";
        let m = parse_frontmatter(md).unwrap();
        assert_eq!(m.name.as_deref(), Some("Hello"));
        assert_eq!(m.description.as_deref(), Some("Does a thing"));
    }

    #[test]
    fn no_frontmatter_is_error() {
        assert_eq!(parse_frontmatter("# Just a heading\n"), Err(ManifestError::NoFrontmatter));
    }

    #[test]
    fn unterminated_frontmatter_is_error() {
        assert_eq!(parse_frontmatter("---\nname: X\n"), Err(ManifestError::Unterminated));
    }

    #[test]
    fn is_skill_requires_name() {
        let with = SkillManifest { name: Some("X".into()), description: None };
        let without = SkillManifest::default();
        assert!(with.is_valid());
        assert!(!without.is_valid());
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --lib manifest`
Expected: compile error — `parse_frontmatter` / `is_valid` not found.

- [ ] **Step 3: Implement the parser**

Add to `src/manifest.rs` above `#[cfg(test)]`:

```rust
impl SkillManifest {
    /// A folder is a skill only if its manifest carries at least a name.
    pub fn is_valid(&self) -> bool {
        self.name.as_ref().is_some_and(|n| !n.trim().is_empty())
    }
}

/// Parse a leading `---`-delimited frontmatter block. Returns an error when the
/// block is absent or unterminated so the caller can report it rather than
/// silently treating the folder as a non-skill.
pub fn parse_frontmatter(content: &str) -> Result<SkillManifest, ManifestError> {
    let trimmed = content.strip_prefix('\u{feff}').unwrap_or(content);
    if !trimmed.starts_with("---") {
        return Err(ManifestError::NoFrontmatter);
    }
    let after = &trimmed[3..];
    let after = after.strip_prefix('\n').or_else(|| after.strip_prefix("\r\n")).unwrap_or(after);
    let end = after.find("\n---").ok_or(ManifestError::Unterminated)?;
    let block = &after[..end];

    let mut manifest = SkillManifest::default();
    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
        match key.trim() {
            "name" => manifest.name = Some(value),
            "description" => manifest.description = Some(value),
            _ => {}
        }
    }
    Ok(manifest)
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test --lib manifest`
Expected: 4 unit tests PASS.

- [ ] **Step 5: Run gates and commit**

```bash
cargo test --lib
git add src/manifest.rs src/lib.rs
git commit -m "feat(manifest): dep-free typed frontmatter parser with errors"
```

---

### Task 7: Rewire scan engine — providers, tightened detection, priority dedup, provenance

This is the integration task. `scan_all` now iterates `DiscoveredRoot`s, parses each candidate with the typed manifest (dropping the fuzzy README rule), collects candidates per skill id across all roots, writes the highest-priority winner plus all sources.

**Files:**
- Modify: `src/scan.rs`
- Test: `tests/skillhub_discovery.rs`

- [ ] **Step 1: Write the failing integration test**

Append to `tests/skillhub_discovery.rs`:

```rust
use std::fs;

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
    assert_eq!(skill.name, "Shared Project"); // higher priority wins
    assert_eq!(skill.source_agent, "project");

    let sources = db.get_skill_sources("shared").unwrap();
    assert_eq!(sources.len(), 2); // both locations recorded
}

#[test]
fn scan_rejects_folders_without_valid_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let (cfg, db) = temp_db(&temp);
    let root = temp.path().join("root");
    // A README mentioning "skill" is no longer enough.
    let dir = root.join("not-a-skill");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.md"), "This mentions a skill and an agent.\n").unwrap();

    let roots = vec![skillhub::providers::DiscoveredRoot {
        agent: "generic".into(),
        path: root,
        priority: skillhub::providers::priority::USER_GLOBAL,
    }];
    let report = skillhub::scan::scan_roots(&cfg, &db, &roots, true).unwrap();
    assert_eq!(report.skills_indexed, 0);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test skillhub_discovery scan_`
Expected: compile error — no function `scan_roots`.

- [ ] **Step 3: Refactor `scan.rs`**

In `src/scan.rs`:

(a0) **Make the home-dir helper crate-visible.** In `src/config.rs`, change `fn home_dir() -> PathBuf {` to `pub(crate) fn home_dir() -> PathBuf {` so `scan.rs` and `mcp.rs` can reuse it (it is currently private; `cli.rs` has its own separate private copy which can stay). All `crate::config::home_dir()` references in this plan depend on this one-line change.

(a) Add imports at the top:

```rust
use crate::manifest::parse_frontmatter;
use crate::providers::DiscoveredRoot;
use std::collections::HashMap;
```

(b) Replace `scan_all` with a thin wrapper plus the new `scan_roots`:

```rust
pub fn scan_all(cfg: &AppConfig, db: &Database) -> Result<ScanReport> {
    let home = crate::config::home_dir();
    let cwd = std::env::current_dir().unwrap_or_else(|_| home.clone());
    let roots = cfg.discovered_roots(&home, &cwd);
    scan_roots(cfg, db, &roots, true)
}

/// Scan the given roots. When `force` is false, roots whose signature is
/// unchanged since the last scan are skipped (incremental).
pub fn scan_roots(
    cfg: &AppConfig,
    db: &Database,
    roots: &[DiscoveredRoot],
    force: bool,
) -> Result<ScanReport> {
    let _ = cfg;
    let mut roots_scanned = 0;
    // id -> (priority, agent, root, inspected)
    let mut winners: HashMap<String, (u8, String, String, InspectedSkill)> = HashMap::new();
    let mut sources: HashMap<String, Vec<(String, String, u8)>> = HashMap::new();

    for root in roots {
        if !root.path.exists() {
            continue;
        }
        let root_key = root.path.to_string_lossy().to_string();
        if !force {
            let signature = root_signature(&root.path);
            if db.get_scan_signature(&root_key)?.as_deref() == Some(signature.as_str()) {
                continue; // unchanged — skip
            }
        }
        roots_scanned += 1;
        for dir in candidate_dirs(&root.path) {
            if let Some(inspected) = inspect_skill_dir(&dir)? {
                let id = inspected.0.id.clone();
                sources.entry(id.clone()).or_default().push((
                    root.agent.clone(),
                    dir.to_string_lossy().to_string(),
                    root.priority,
                ));
                let better = match winners.get(&id) {
                    Some((p, _, _, _)) => root.priority > *p,
                    None => true,
                };
                if better {
                    winners.insert(
                        id,
                        (root.priority, root.agent.clone(), root_key.clone(), inspected),
                    );
                }
            }
        }
        db.set_scan_signature(&root_key, &root_signature(&root.path))?;
    }

    let skills_indexed = winners.len();
    for (id, (_priority, agent, root_key, (mut skill, files, commands))) in winners {
        skill.source_agent = agent;
        skill.source_root = root_key;
        db.upsert_skill(&skill, &files, &commands)?;
        if let Some(found) = sources.get(&id) {
            db.replace_skill_sources(&id, found)?;
        }
    }

    Ok(ScanReport {
        roots_scanned,
        skills_indexed,
    })
}

/// Cheap signature for a root: its mtime plus the mtimes of its immediate
/// subdirectories. Stat-only — never walks the full tree.
fn root_signature(root: &Path) -> String {
    use std::time::UNIX_EPOCH;
    let mut parts: Vec<String> = Vec::new();
    let mtime = |p: &Path| -> u64 {
        p.metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0)
    };
    parts.push(mtime(root).to_string());
    if let Ok(entries) = fs::read_dir(root) {
        let mut subs: Vec<(String, u64)> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| (e.file_name().to_string_lossy().to_string(), mtime(&e.path())))
            .collect();
        subs.sort();
        for (name, t) in subs {
            parts.push(format!("{name}:{t}"));
        }
    }
    parts.join("|")
}
```

(c) Rewrite `inspect_skill_dir`'s gate to use the typed manifest and drop the fuzzy README rule. Replace the block from `let skill_md = dir.join("SKILL.md");` through the early `return Ok(None);` with:

```rust
    let skill_md = dir.join("SKILL.md");
    let skill_yaml = dir.join("skill.yaml");
    let readme = dir.join("README.md");
    let readme_text = read_if_exists(&readme)?;

    let manifest_text = read_if_exists(&skill_md)?.or(read_if_exists(&skill_yaml)?);
    let manifest = match manifest_text.as_deref().map(parse_frontmatter) {
        Some(Ok(m)) if m.is_valid() => m,
        _ => return Ok(None), // no valid SKILL.md/skill.yaml manifest => not a skill
    };
    let skill_text = read_if_exists(&skill_md)?;
```

Then change the `name`/`summary` derivation to prefer the parsed manifest:

```rust
    let name = manifest
        .name
        .clone()
        .or_else(|| skill_text.as_deref().and_then(extract_heading))
        .unwrap_or_else(|| id.clone());
    let summary = manifest
        .description
        .clone()
        .or_else(|| readme_text.as_deref().and_then(extract_first_paragraph))
        .unwrap_or_else(|| "No summary available.".to_string());
```

(Remove the now-unused `has_readme_keyword` logic and the old `extract_frontmatter_field`/`extract_frontmatter_description` calls for name/summary. Leave `extract_heading`, `extract_first_paragraph`, and other helpers — they are still used. If clippy reports a helper as dead, delete it.)

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test --test skillhub_discovery`
Expected: all 4 discovery tests PASS.

- [ ] **Step 5: Run the full suite (expect fixture-count breakage to fix next)**

Run: `cargo test`
Expected: `skillhub_core`/`skillhub_control_plane` may fail on skill counts because tightened detection changes what counts as a skill in the fixtures. Note which counts changed; fix in Task 8 Step 1. If they pass, even better.

- [ ] **Step 6: Commit**

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings
git add src/scan.rs tests/skillhub_discovery.rs
git commit -m "feat(scan): provider-driven scan with priority dedup and provenance"
```

---

### Task 8: Incremental skip + `scan --force` CLI + fixture counts

**Files:**
- Modify: `src/cli.rs` (add `--force` to `Scan`)
- Modify: `tests/skillhub_core.rs`, `tests/skillhub_control_plane.rs` (counts if changed)
- Test: `tests/skillhub_discovery.rs`

- [ ] **Step 1: Reconcile fixture counts**

If Task 7 Step 5 changed counts, update the literal assertions. Check the fixtures under `tests/fixtures/skills/` — every fixture that should still count must have a valid `SKILL.md` frontmatter with a `name`. The v0.4 fixtures (`docs-heavy`, `danger-tool`, the clean one, etc.) already have `name:` frontmatter, so the indexed total should be unchanged at **4**. If a fixture relied on the README-keyword rule, either add proper frontmatter to it (preferred — keep the count) or update the count. Re-run:

Run: `cargo test`
Expected: all pass after reconciling counts.

- [ ] **Step 2: Write the incremental test**

Append to `tests/skillhub_discovery.rs`:

```rust
#[test]
fn incremental_scan_skips_unchanged_roots() {
    let temp = tempfile::tempdir().unwrap();
    let (cfg, db) = temp_db(&temp);
    let root = temp.path().join("r");
    write_skill(&root, "one", "One");
    let roots = vec![skillhub::providers::DiscoveredRoot {
        agent: "generic".into(),
        path: root.clone(),
        priority: skillhub::providers::priority::USER_GLOBAL,
    }];

    let first = skillhub::scan::scan_roots(&cfg, &db, &roots, false).unwrap();
    assert_eq!(first.roots_scanned, 1);

    // No change → root skipped on the next non-forced scan.
    let second = skillhub::scan::scan_roots(&cfg, &db, &roots, false).unwrap();
    assert_eq!(second.roots_scanned, 0);
}
```

- [ ] **Step 3: Run it to verify it passes**

Run: `cargo test --test skillhub_discovery incremental_scan_skips_unchanged_roots`
Expected: PASS (logic already implemented in Task 7).

- [ ] **Step 4: Add `--force` to the Scan subcommand**

In `src/cli.rs`, find the `Scan` variant in `enum Command`. If it is a unit variant `Scan,`, change it to:

```rust
    Scan {
        #[arg(long)]
        force: bool,
    },
```

In the `match cli.command` arm for `Command::Scan`, replace the body to call the registry and `scan_roots`:

```rust
        Command::Scan { force } => {
            let home = home_dir();
            let cwd = std::env::current_dir().unwrap_or_else(|_| home.clone());
            let roots = cfg.discovered_roots(&home, &cwd);
            let report = scan::scan_roots(&cfg, &db, &roots, force)?;
            println!(
                "Scanned {} roots, indexed {} skills",
                report.roots_scanned, report.skills_indexed
            );
        }
```

(If the existing arm already prints differently, keep its message but route through `scan_roots` with `force`. Ensure `home_dir` is already imported in `cli.rs` — it is used by the v0.4 `Connect` arm.)

- [ ] **Step 5: Run gates and commit**

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add src/cli.rs tests/
git commit -m "feat(scan): incremental skip and scan --force"
```

---

### Task 9: MCP lazy freshness + `source_agent` in output

**Files:**
- Modify: `src/mcp.rs`
- Test: `tests/skillhub_control_plane.rs`

- [ ] **Step 1: Read the current MCP list/search handlers**

Run: `cargo run -- help` is not needed; instead open `src/mcp.rs` and locate where `list_skills` / `search_skills` build their JSON from `db.list_skills()` / `db.search_skills(..)`. Note the function that handles `tools/call` and the per-skill JSON object builder.

- [ ] **Step 2: Write the failing test**

In `tests/skillhub_control_plane.rs`, inside the existing `mcp_hides_and_refuses_blocked_skills` test (or a new test `mcp_list_includes_source_agent`), after obtaining the listed skills JSON, assert each visible skill object has a `source_agent` string field:

```rust
#[test]
fn mcp_list_includes_source_agent() {
    let temp = tempfile::tempdir().unwrap();
    let (cfg, db) = scanned_db(&temp);
    let req = serde_json::json!({
        "jsonrpc":"2.0","id":1,"method":"tools/call",
        "params": {"name":"skillhub.list_skills","arguments":{}}
    });
    let result = skillhub::mcp::handle_request(&cfg, &db, &req).unwrap();
    let text = result["content"][0]["text"].as_str().unwrap();
    let payload: serde_json::Value = serde_json::from_str(text).unwrap();
    let first = &payload["skills"][0];
    assert!(first["source_agent"].is_string());
}
```

(Reuse the `scanned_db` helper already in that file. If it scans fixtures via `scan_all`, the provenance will be populated.)

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test --test skillhub_control_plane mcp_list_includes_source_agent`
Expected: FAIL — `source_agent` missing from the JSON.

- [ ] **Step 4: Add the field and the lazy freshness check**

In `src/mcp.rs`:

(a) In the per-skill JSON object builder for `list_skills`/`search_skills`, add the field:

```rust
        "source_agent": skill.source_agent,
```

(b) Add a freshness pre-step. At the start of the `list_skills`/`search_skills` handling (before querying), insert a cheap re-scan of stale roots:

```rust
    refresh_stale_roots(cfg, db)?;
```

and define near the top of `mcp.rs`:

```rust
/// Cheap, O(roots) freshness check: re-scan only roots whose signature changed
/// since the last scan. Never walks a tree unless a root is actually stale.
fn refresh_stale_roots(cfg: &AppConfig, db: &Database) -> Result<()> {
    let home = crate::config::home_dir();
    let cwd = std::env::current_dir().unwrap_or_else(|_| home.clone());
    let roots = cfg.discovered_roots(&home, &cwd);
    crate::scan::scan_roots(cfg, db, &roots, false)?; // force=false => only stale roots re-walked
    Ok(())
}
```

(`scan_roots` with `force=false` already skips unchanged roots via signature, so this is the required cheap path. Ensure `use crate::scan;` / `AppConfig` / `Database` are in scope — add imports as needed.)

- [ ] **Step 5: Run it to verify it passes**

Run: `cargo test --test skillhub_control_plane`
Expected: all control-plane tests PASS, including the new one.

- [ ] **Step 6: Run gates and commit**

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git add src/mcp.rs tests/skillhub_control_plane.rs
git commit -m "feat(mcp): lazy freshness re-scan and source_agent in list output"
```

---

### Task 10: Real-world verification, detection diff, release

**Files:**
- Modify: `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `ROADMAP.md`, `README.md`, `README.zh-CN.md`

- [ ] **Step 1: Real-library before/after detection diff**

```bash
cargo build
./target/debug/skillhub scan --force
./target/debug/skillhub audit > /tmp/v05-skills.txt
```

Compare the indexed skill set against the v0.4 baseline (128 on the maintainer's machine). For every skill that **disappeared**, confirm it lacked a valid `SKILL.md`/`skill.yaml` manifest (i.e., it only matched the old fuzzy README rule) and is genuinely not a skill. List them. If a real skill dropped, STOP — its packaging needs a manifest or the detector needs adjustment. This is the release criterion for the detection change.

- [ ] **Step 2: Spot-check provenance + freshness manually**

```bash
./target/debug/skillhub show <some-skill> --json   # confirm source_agent populated
# add a new skill dir under a scanned root, then:
./target/debug/skillhub scan                        # non-forced: only that root re-walked
```

Confirm the new skill appears without `--force` (lazy/incremental works), and that re-running `scan` with no changes reports `Scanned 0 roots`.

- [ ] **Step 3: Version, changelog, roadmap**

`Cargo.toml`: `version = "0.4.0"` → `version = "0.5.0"`. Run `cargo build` to sync `Cargo.lock`.

At the top of `CHANGELOG.md` after `# Changelog`:

```markdown
## 0.5.0 - YYYY-MM-DD

Discovery release: broad, fresh, deterministic skill discovery.

- Per-OS provider registry replaces hardcoded scan roots. SkillHub now finds skills from Claude (incl. plugin skills), Cursor, Codex, generic XDG locations, and the current project — expanded correctly on Windows/macOS/Linux.
- Skill provenance: each skill records which agent it came from (`source_agent`), surfaced in `show` and MCP `list_skills`. Duplicate skill ids are resolved deterministically by priority (user-config > project > user-global > plugin) and every location is recorded.
- Lazy incremental re-scan: `scan` skips unchanged roots; MCP `list_skills`/`search_skills` re-scan only stale roots on access, so the index is never stale — with no resident daemon. New `scan --force` for a full re-scan.
- Tightened detection: a folder is a skill only with a valid `SKILL.md`/`skill.yaml` manifest; the fuzzy README heuristic was removed, cutting false positives.
```

In `ROADMAP.md`, move the current `## v0.5` content to `## v0.6` and write the new `## v0.5`:

```markdown
## v0.5

- Per-OS provider registry and cross-agent skill discovery.
- Skill provenance and deterministic priority de-duplication.
- Lazy incremental re-scan (fresh on access, no daemon).
- Tightened manifest-based detection.
```

(Note in `## v0.6`: `scan --deep`, `skillhub roots` management, `connect` auto-adds scan roots, `skillhub watch`, README GIF.)

- [ ] **Step 4: README updates (EN + 中文)**

In `README.md` `## Trust and Audit` (or a new `## Discovery` section), describe provider-based discovery, provenance, and lazy refresh. Add to the `## CLI` block:

```text
skillhub scan [--force]
```

Mirror the same additions in `README.zh-CN.md` (新增「发现」说明 + `scan [--force]`). Keep both languages telling the same story (v0.4 self-review item #5).

- [ ] **Step 5: Final gates**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all clean, all tests pass.

- [ ] **Step 6: Commit and open the PR**

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md ROADMAP.md README.md README.zh-CN.md
git commit -m "chore: release v0.5.0"
git push -u origin feat/v0.5-discovery
gh pr create --base feat/v0.4-adoption --title "SkillHub v0.5: cross-agent discovery engine" --body "<summary: provider registry + per-OS paths, provenance + priority dedup, lazy incremental re-scan (no daemon), tightened detection. Include the before/after detection diff from Task 10 Step 1. List verification gate results.>"
```

(Base on `feat/v0.4-adoption` until v0.4 merges, so the diff shows only v0.5 work.)

---

## Self-review checklist (for the executing agent, after Task 10)

1. **Breadth proven.** Did discovery actually find skills outside the old four roots (e.g. a plugin skill, a project `.skills`)? Show it.
2. **No real skill dropped.** Task 10 Step 1 diff: every disappeared skill genuinely lacked a manifest.
3. **Freshness without daemon.** New skill appears on a plain `scan` (and via MCP) without `--force`; unchanged re-scan reports 0 roots; no background process exists.
4. **Determinism.** Same skill in two roots always resolves to the higher-priority copy; `skill_sources` lists both.
5. **Compatibility.** `audit --json` / `show --json` / MCP `get_skill` shapes are additive (gained `source_agent`); existing `config.toml` still loads; DB migrated in place.
6. **Freshness is cheap.** Confirm the MCP path does not full-walk every call — only stale roots are re-walked (signature is stat-only).
