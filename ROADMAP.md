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

- Reposition SkillHub as a local control plane for AI skills.
- Add deterministic local skill audits (`skillhub audit`).
- Add per-skill trust decisions that gate MCP visibility (`skillhub trust`).
- Add agent integration diagnostics (`skillhub doctor agents|codex|claude|cursor`).
- Surface trust/audit/visibility metadata in `show` and MCP `get_skill`.

## v0.4

- Context-aware audit rules v2 with far fewer false positives.
- One-command agent onboarding (`skillhub connect`).
- Sandboxed `skillhub demo` and README demo GIF.

## v0.5

- Per-OS provider registry and cross-agent skill discovery.
- Skill provenance and deterministic priority de-duplication.
- Lazy incremental re-scan (fresh on access, no daemon).
- Tightened manifest-based detection.

## v0.6

- Add `scan --deep` for opt-in whole-home discovery.
- Add `skillhub roots list/add/remove` management commands.
- Let `connect <agent>` register that agent's skill directory as a scan root.
- Add `skillhub watch` for optional file-watching mode.
- Render and publish the README demo GIF.
- Switch MCP server internals to the official Rust MCP SDK.
- Add version pinning, lockfile metadata, checksums, and stronger supply-chain reports.
- Add release binaries for Windows, macOS, and Linux.

## Later

- Optional command execution with explicit approvals.
- Optional local web UI.
- Skill marketplace/index integration.
- Team policy profiles.
