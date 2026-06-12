# Roadmap

## v0.1 Alpha

- Local skill scanning
- GitHub skill install
- SQLite index
- CLI search/show/doctor
- Minimal MCP stdio server

## v0.2

- Add first-run `setup`.
- Add agent instruction snippets.
- Add capability detection and better search.
- Add dry-run command previews.
- Improve `show` output for humans.

## v0.3

- Switch MCP server internals to the official Rust MCP SDK.
- Improve parsing for `skill.yaml` and common skill frontmatter.
- Add `skillhub doctor --fix-suggestions`.

## v0.4

- Add version pinning for installed GitHub skills.
- Add lockfile metadata for skill provenance.
- Add checksum and safety scan reports.
- Add release binaries for Windows, macOS, and Linux.

## Later

- Optional command execution with explicit approvals.
- Optional local web UI.
- Skill marketplace/index integration.
- Team policy profiles.
