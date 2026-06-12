# SkillHub

**One local skill library for every AI agent.**

SkillHub is a lightweight Rust CLI and MCP server that lets Codex, Claude Code, Cursor, OpenCode, and other MCP-capable agents share the same local agent skills.

It scans your existing skill folders, indexes `SKILL.md` packages, installs skills from GitHub, and exposes them through one MCP server.

```text
Codex / Claude Code / Cursor / OpenCode
        |
        | MCP
        v
SkillHub
        |
        +-- ~/.agents/skills
        +-- ~/.claude/skills
        +-- ~/.codex/skills
        +-- ./.skills
```

## Why

AI agent skills are becoming reusable packages: instructions, scripts, references, templates, and troubleshooting notes. The problem is that every agent stores and discovers them differently.

SkillHub gives you one local registry:

- Find skills installed by another agent.
- Search local skills from any MCP-capable agent.
- Read `SKILL.md`, `README.md`, and `runtime.conf` on demand.
- See recommended commands without executing them.
- Diagnose missing runtime or environment variables.
- Keep v1 safe: no silent script execution.

## Install

### From source

```bash
git clone https://github.com/RsLuna7/skillhub.git
cd skillhub
cargo install --path .
```

### Run from a checkout

```bash
cargo build
./target/debug/skillhub init
./target/debug/skillhub scan
./target/debug/skillhub list
```

On Windows PowerShell:

```powershell
cargo build
.\target\debug\skillhub.exe init
.\target\debug\skillhub.exe scan
.\target\debug\skillhub.exe list
```

## Quick Start

Install a skill from GitHub:

```bash
skillhub install anysearch-ai/anysearch-skill
```

Search your skills:

```bash
skillhub search "web search"
```

Inspect a skill:

```bash
skillhub show anysearch-skill
skillhub doctor anysearch-skill --json
```

Print MCP config:

```bash
skillhub mcp-config
```

## Connect to Codex

Add SkillHub as a Codex MCP server:

```bash
codex mcp add skillhub -- skillhub mcp
```

Or add this to `~/.codex/config.toml`:

```toml
[mcp_servers.skillhub]
command = "skillhub"
args = ["mcp"]
```

Then restart Codex and ask:

```text
Use skillhub to list my local skills.
```

## Connect to Other Agents

Any MCP-capable agent can use SkillHub with this server config:

```json
{
  "mcpServers": {
    "skillhub": {
      "command": "skillhub",
      "args": ["mcp"]
    }
  }
}
```

If `skillhub` is not on PATH, use the absolute path to the binary.

More examples:

- [Codex setup](docs/codex.md)
- [Claude setup](docs/claude.md)
- [Cursor setup](docs/cursor.md)
- [MCP tool reference](docs/mcp.md)

## CLI

```text
skillhub init
skillhub scan
skillhub list
skillhub search <query>
skillhub show <skill-id>
skillhub doctor [skill-id] [--json]
skillhub install <owner/repo | github-url>
skillhub mcp
skillhub mcp-config
skillhub config paths
skillhub config add-path <path>
```

## MCP Tools

`skillhub mcp` exposes:

```text
skillhub.search_skills
skillhub.list_skills
skillhub.get_skill
skillhub.get_skill_file
skillhub.get_skill_commands
skillhub.doctor_skill
```

## Defaults

```text
Data:    ~/.skillhub
Index:   ~/.skillhub/index.sqlite
Config:  ~/.skillhub/config.toml
Install: ~/.agents/skills
```

Default scan roots:

```text
~/.agents/skills
~/.claude/skills
~/.codex/skills
./.skills
```

Add another folder:

```bash
skillhub config add-path /path/to/skills
skillhub scan
```

## Safety Model

SkillHub v1 is read-first and intentionally conservative.

- It indexes and reads skills.
- It returns recommended commands.
- It does not execute skill scripts.
- It blocks `.env`, hidden files, absolute paths, and path traversal in MCP file reads.
- GitHub install refuses to overwrite existing skill directories.

This keeps SkillHub useful as a shared registry without becoming a silent code execution layer.

## Current Status

SkillHub is alpha software. It is ready for local experimentation and feedback.

Implemented:

- Rust single-binary CLI
- SQLite local index
- Local skill scanning
- GitHub skill install
- Skill search/show/doctor
- Minimal MCP stdio server
- Safety checks for MCP file reads

Planned:

- Official MCP SDK implementation
- Better full-text search
- Version pinning and lockfiles
- Stronger supply-chain checks for GitHub skills
- Release binaries for Windows, macOS, and Linux

See [ROADMAP.md](ROADMAP.md).

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo check
```

## License

MIT
