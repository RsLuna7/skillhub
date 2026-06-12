# Changelog

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
