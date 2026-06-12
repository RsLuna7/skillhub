# SkillHub

**One local skill library for your AI tools.**

SkillHub is a lightweight Rust CLI and MCP server that lets Codex, Claude Code, Cursor, OpenCode, and other MCP-capable tools share the same local skills.

It scans your existing skill folders, indexes `SKILL.md` packages, installs skills from GitHub, and exposes them through one MCP server.

## Quick Start

```bash
cargo install --git https://github.com/RsLuna7/skillhub
skillhub setup
```

Connect it to any MCP client:

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

Try a general-purpose search:

```text
Use skillhub to find a template skill.
```

## Why

Skills are becoming reusable packages: instructions, scripts, references, templates, and troubleshooting notes. The problem is that every AI tool stores and discovers them differently.

SkillHub gives you one local registry:

- Find skills installed by another tool.
- Search local skills from any MCP-capable client.
- Read `SKILL.md`, `README.md`, and `runtime.conf` on demand.
- See recommended commands without executing them.
- Preview command execution with a dry-run.
- Diagnose missing runtime or environment variables.
- Keep v0.2 safe: no silent script execution.

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

## Demo

```bash
$ skillhub setup
SkillHub setup complete
MCP config:
{
  "mcpServers": {
    "skillhub": {
      "command": "skillhub",
      "args": ["mcp"]
    }
  }
}

$ skillhub install owner/template-skill
Installed: ~/.agents/skills/template-skill

$ skillhub search "template"
ID                       Name              Risk     Capabilities      Summary
template-skill           Template Helper   low      writing           Reusable templates...

$ skillhub show template-skill
What it does
  Reusable templates for common writing and planning workflows.

How to use it
  Read SKILL.md first, then inspect commands if needed.

$ skillhub run template-skill 1 --dry-run
will_execute: false
```

## Install

From GitHub:

```bash
cargo install --git https://github.com/RsLuna7/skillhub
```

From source:

```bash
git clone https://github.com/RsLuna7/skillhub.git
cd skillhub
cargo install --path .
```

Or download a binary from [Releases](https://github.com/RsLuna7/skillhub/releases).

## Setup

Run:

```bash
skillhub setup
```

This initializes SkillHub, scans configured skill folders, runs diagnostics, and prints MCP config snippets. It does **not** edit Codex, Claude, or Cursor config files.

Print reusable agent instructions:

```bash
skillhub agent-instructions
skillhub agent-instructions codex
```

## Connect to Codex

```bash
codex mcp add skillhub -- skillhub mcp
```

Or add this to `~/.codex/config.toml`:

```toml
[mcp_servers.skillhub]
command = "skillhub"
args = ["mcp"]
```

Then restart Codex and try a normal skill lookup:

```text
Use skillhub to find a writing template.
```

## CLI

```text
skillhub setup
skillhub init
skillhub scan
skillhub list
skillhub search <query> [--json]
skillhub show <skill-id> [--json]
skillhub run <skill-id> <command-index> --dry-run
skillhub doctor [skill-id] [--json]
skillhub install <owner/repo | github-url>
skillhub agent-instructions [codex|claude|cursor]
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

SkillHub v0.2 is read-first and intentionally conservative.

- It indexes and reads skills.
- It returns recommended commands.
- `skillhub run` is dry-run only.
- It does not execute skill scripts.
- It blocks `.env`, hidden files, absolute paths, and path traversal in MCP file reads.
- GitHub install refuses to overwrite existing skill directories.

## Current Status

SkillHub is alpha software. It is ready for local experimentation and feedback.

Implemented:

- Rust single-binary CLI
- SQLite local index with FTS5 fallback search
- Local skill scanning with capability detection
- GitHub skill install
- `setup`, `search`, `show`, `doctor`, and dry-run command previews
- Minimal MCP stdio server
- Safety checks for MCP file reads

Planned:

- Official MCP SDK implementation
- Version pinning and lockfiles
- Stronger supply-chain checks for GitHub skills
- Optional approval-based command execution

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
