# Changelog

## 0.5.0 - 2026-06-12

Discovery release: broad, fresh, deterministic skill discovery.

- Per-OS provider registry replaces hardcoded scan roots. SkillHub now finds skills from Claude (including plugin skills), Cursor, Codex, generic XDG locations, and the current project, expanded correctly on Windows, macOS, and Linux.
- Skill provenance: each skill records which agent it came from (`source_agent`), surfaced in `show` and MCP `list_skills`. Duplicate skill ids are resolved deterministically by priority (user-config > project > user-global > plugin) and every location is recorded.
- Lazy incremental re-scan: `scan` skips unchanged roots; MCP `list_skills`/`search_skills` re-scan stale indexed roots on access, so the index stays fresh with no resident daemon. New `scan --force` performs a full re-scan.
- Tightened detection: a folder is a skill only with a valid `SKILL.md`/`skill.yaml` manifest; the fuzzy README heuristic was removed, cutting false positives.
- Local Web UI: `skillhub ui` opens a browser dashboard for scanning, browsing, auditing, and trust/block management without executing skill scripts.

## 0.4.0 - 2026-06-12

Adoption release: trustworthy audits, one-command onboarding, and an instant demo.

- Audit rules v2: context-aware scanning. Markdown prose is never scanned, markdown code blocks are scanned at downgraded severity, and `rm` deletes are only critical when targeting system paths. Documentation can at worst `warn`, never `fail`.
- Add `skillhub connect codex|claude|cursor [--dry-run]` to register the MCP server in agent configs, with automatic backups.
- Add `skillhub demo`, a sandboxed 30-second guided tour (scan → audit → block → MCP view) that touches nothing outside a temp folder.
- Add a VHS tape (`demo/demo.tape`) and README demo GIF.


## 0.3.0 - 2026-06-12

SkillHub is now a local control plane for AI skills: you decide which skills agents may see.

- Add `skillhub audit [skill-id]` for deterministic, offline static audits of skill files; results persist in SQLite and surface in `show` and MCP `get_skill`.
- Add `skillhub trust list|allow|block|reset` for per-skill trust decisions.
- Hide blocked skills from MCP `list_skills`/`search_skills` and refuse them in `get_skill`, `get_skill_file`, `get_skill_commands`, and `doctor_skill`.
- Add `skillhub doctor agents|codex|claude|cursor` for agent integration diagnostics.
- Add trust/audit/visibility metadata to `skillhub show` and MCP `get_skill`.
- Add SQLite migrations for the new `skill_trust` and `audit_results` tables; v0.2 databases upgrade in place.

## 0.2.0 - 2026-06-12

- Add `skillhub setup` for first-run initialization, scan, diagnostics, and MCP snippets.
- Add `skillhub agent-instructions` for reusable agent guidance.
- Add `skillhub run <skill-id> <command-index> --dry-run` for safe command previews.
- Add `search --json` and `show --json`.
- Improve `show` output around how to use a skill.
- Add detected capabilities and improved search for web-search style queries.
- Add MCP `get_skill` next-action guidance.

## 0.1.0 - 2026-06-12

Initial alpha release.

- Add Rust CLI for local agent skill discovery.
- Add SQLite index and scanner for `SKILL.md` folders.
- Add GitHub skill installation from `owner/repo` or GitHub URLs.
- Add `list`, `search`, `show`, and `doctor` commands.
- Add minimal MCP stdio server with skill discovery tools.
- Add safe skill file reads that block `.env`, hidden files, absolute paths, and path traversal.
