# Changelog

## 0.1.0 - 2026-06-12

Initial alpha release.

- Add Rust CLI for local agent skill discovery.
- Add SQLite index and scanner for `SKILL.md` folders.
- Add GitHub skill installation from `owner/repo` or GitHub URLs.
- Add `list`, `search`, `show`, and `doctor` commands.
- Add minimal MCP stdio server with skill discovery tools.
- Add safe skill file reads that block `.env`, hidden files, absolute paths, and path traversal.
