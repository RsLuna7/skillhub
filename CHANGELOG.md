# Changelog

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
