# SkillHub v0.3: Local Control Plane for AI Skills

## Context

SkillHub v0.2 is a read-first local skill registry: it scans, indexes, and serves
skills to MCP-capable agents without executing anything. v0.3 repositions SkillHub
as a **local control plane for AI skills**: the place where a human decides which
skills agents may see and use, backed by deterministic local auditing.

v0.3 stays a Rust single-binary CLI + MCP server. No command execution, no web UI,
no cloud services. All audit logic is local, deterministic, and testable.

## Goals

1. `skillhub audit [skill-id] [--json]` — deterministic static audit of skill files.
2. `skillhub trust <allow|block|reset|list>` — local trust decisions per skill.
3. `skillhub doctor agents|codex|claude|cursor` — agent integration diagnostics.
4. `show` and MCP `get_skill` include trust/audit/visibility metadata.
5. Blocked skills are invisible/refused over MCP; the local CLI still sees them.
6. SQLite migrations for the new tables; v0.2 databases upgrade in place.

## Non-Goals

- No real command execution (`run` stays dry-run only).
- No network calls during audit; rules run on local files only.
- No changes that break v0.2 CLI invocations or JSON shapes (additive only).

## Design

### Audit (`src/audit.rs`, new)

A rule engine that scans a skill's indexed text files (`SKILL.md`, `README.md`,
`runtime.conf`, `skill.yaml`, `.env.example`, `scripts/*`) line by line.

Rules (version `v1`, all case-insensitive, deterministic ordering by file then line):

| Rule | Severity | Trigger |
|------|----------|---------|
| `remote-script-execution` | high | `curl`/`wget`/`iwr`/`Invoke-WebRequest` piped into `sh`/`bash`/`iex`/`powershell` |
| `destructive-delete` | high | `rm -rf`, `Remove-Item ... -Recurse ... -Force`, `del /f`, `rmdir /s` |
| `dynamic-execution` | high | `eval(`, `Invoke-Expression`, standalone `iex` |
| `hardcoded-secret` | high | `KEY/TOKEN/SECRET/PASSWORD = "<long literal>"` outside `.env.example` (excerpt redacted) |
| `privilege-escalation` | medium | `sudo` |
| `obfuscation` | medium | `base64 -d`, `FromBase64String`, `b64decode` |
| `insecure-http` | low | `http://` URLs (localhost exempt) |

Report status: `fail` if any high finding, `warn` if any medium/low finding,
`pass` otherwise. Reports are persisted to `audit_results` (one row per skill,
latest wins) so `show`/MCP can surface them without re-scanning.

### Trust (`src/trust.rs`, new)

Trust state per skill, stored in `skill_trust`:

- `trusted` — explicitly allowed by the user.
- `untrusted` — default for any skill without a row.
- `blocked` — explicitly blocked; hidden from MCP.

CLI:

```text
skillhub trust list [--json]
skillhub trust allow <skill-id>
skillhub trust block <skill-id> [--reason <text>]
skillhub trust reset <skill-id>
```

Visibility is derived: `blocked` ⇒ `blocked`, anything else ⇒ `visible`.

### MCP enforcement (`src/mcp.rs`)

- `list_skills` / `search_skills` exclude blocked skills.
- `get_skill`, `get_skill_file`, `get_skill_commands` return an error
  (`blocked by local trust policy`) for blocked skills.
- `get_skill` payload (the usage summary) gains `trust`, `trust_reason`,
  `audit`, and `visibility` fields.
- `handle_request` becomes public so tests can drive the JSON-RPC surface
  without spawning a process.

### Agent diagnostics (`src/doctor.rs`)

`doctor_agent(home, agent)` checks, per agent, using an injectable home dir so
tests are hermetic:

- **codex**: `~/.codex/config.toml` exists and mentions `skillhub`; `~/.codex/skills` exists.
- **claude**: `~/.claude` exists; `~/.claude.json` or `~/.claude/settings.json` mentions `skillhub`; `~/.claude/skills` exists.
- **cursor**: `~/.cursor/mcp.json` exists and mentions `skillhub`.
- all agents: `skillhub` binary resolvable on `PATH`.

CLI keeps `skillhub doctor [skill-id]` working: the positional argument values
`agents`, `codex`, `claude`, `cursor` route to agent diagnostics; anything else
is treated as a skill id as before.

### Database (`src/db.rs`)

New tables added in `migrate()` (idempotent, `CREATE TABLE IF NOT EXISTS`):

```sql
CREATE TABLE IF NOT EXISTS skill_trust (
    skill_id   TEXT PRIMARY KEY,
    status     TEXT NOT NULL,
    reason     TEXT,
    decided_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS audit_results (
    skill_id      TEXT PRIMARY KEY,
    status        TEXT NOT NULL,
    rules_version TEXT NOT NULL,
    findings_json TEXT NOT NULL,
    audited_at    TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
```

### Surface metadata (`src/skill.rs`, `src/search.rs`)

`SkillUsageSummary` gains additive fields: `trust`, `trust_reason`, `audit`
(status, finding count, audited_at; `null` when never audited), `visibility`.
`print_show` gains Trust/Audit/Visibility lines in the safety section.
Version-pinned "v0.2" strings become version-neutral.

## Files

- New: `src/audit.rs`, `src/trust.rs`, `tests/fixtures/skills/danger-tool/**`.
- Modified: `src/lib.rs`, `src/cli.rs`, `src/db.rs`, `src/doctor.rs`, `src/mcp.rs`,
  `src/search.rs`, `src/skill.rs`, `src/scan.rs` (version-neutral string),
  `src/run.rs` (version-neutral string), `tests/skillhub_core.rs`,
  `Cargo.toml` (0.3.0, rusqlite dev-dependency for the migration test),
  `README.md`, `README.zh-CN.md`, `CHANGELOG.md`, `ROADMAP.md`.

## Tests

1. **Audit**: the `danger-tool` fixture yields deterministic high findings
   (`remote-script-execution`, `destructive-delete`) and `fail`; a clean fixture
   (`writing-helper`) yields `pass`. Reports persist and reload from SQLite.
2. **Trust**: allow/block/reset round-trips; default is `untrusted`.
3. **Blocked MCP behavior**: via `mcp::handle_request` — blocked skills disappear
   from `list_skills`/`search_skills`; `get_skill`/`get_skill_file`/
   `get_skill_commands` return errors; unblocking restores access.
4. **Metadata**: `usage_summary` carries trust/audit/visibility.
5. **Agent diagnostics**: hermetic temp home with fake `~/.codex/config.toml`
   etc.; reports flag present/missing integration.
6. **DB migration**: build a database file with the v0.2 schema plus a skill row,
   run `migrate()`, assert the row survives and `skill_trust`/`audit_results`
   exist and work.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
