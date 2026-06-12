# SkillHub

[![English](https://img.shields.io/badge/lang-English-blue.svg)](README.md)
[![简体中文](https://img.shields.io/badge/lang-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-red.svg)](README.zh-CN.md)

**A local control plane for AI skills.**

SkillHub is a lightweight Rust CLI and MCP server that lets Codex, Claude Code, Cursor, OpenCode, and other MCP-capable tools share the same local skills — and lets you decide which skills those agents may see.

It scans your existing skill folders, indexes `SKILL.md` packages, installs skills from GitHub, audits them with deterministic local rules, and exposes them through one trust-aware MCP server.

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

And once agents can discover skills, someone has to decide which skills they should trust. That decision belongs on your machine, not in a cloud service.

SkillHub gives you one local registry and control plane:

- Find skills installed by another tool.
- Search local skills from any MCP-capable client.
- Read `SKILL.md`, `README.md`, and `runtime.conf` on demand.
- Audit skills with deterministic, local static-analysis rules.
- Allow or block skills per machine; blocked skills disappear from MCP.
- See recommended commands without executing them.
- Preview command execution with a dry-run.
- Diagnose missing runtime or environment variables, and check agent integrations.
- Stay safe by default: no silent script execution.

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

$ skillhub audit template-skill
template-skill: pass (0 findings, rules v1)

$ skillhub trust block risky-skill --reason "review pending"
risky-skill: blocked
Hidden from MCP clients until unblocked.

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
skillhub audit [skill-id] [--json]
skillhub trust list [--json]
skillhub trust allow <skill-id>
skillhub trust block <skill-id> [--reason <text>]
skillhub trust reset <skill-id>
skillhub doctor [skill-id] [--json]
skillhub doctor agents|codex|claude|cursor [--json]
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

MCP results respect your local trust decisions: blocked skills are excluded from `list_skills`/`search_skills`, and the other tools refuse them. `get_skill` includes `trust`, `audit`, and `visibility` metadata so agents can see how much a skill has been vetted.

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

## Trust and Audit

SkillHub v0.3 adds a local control plane on top of the registry.

Audit a skill (or everything) with deterministic, offline rules:

```bash
skillhub audit
skillhub audit some-skill --json
```

The audit flags patterns like piping downloads into a shell, destructive deletes, dynamic code execution, hardcoded secrets, `sudo`, obfuscation, and plain-HTTP endpoints. Results are stored in the local SQLite index and surface in `skillhub show` and MCP `get_skill`.

Decide what agents may see:

```bash
skillhub trust list
skillhub trust allow some-skill
skillhub trust block some-skill --reason "review pending"
skillhub trust reset some-skill
```

Blocked skills stay visible to you in the CLI but are hidden from and refused to MCP clients. Everything is local: no cloud service, no telemetry, no network calls during audit.

## Safety Model

SkillHub is read-first and intentionally conservative.

- It indexes and reads skills.
- It returns recommended commands.
- `skillhub run` is dry-run only.
- It does not execute skill scripts.
- Audits are local, deterministic, and offline.
- Blocked skills are hidden from MCP clients.
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
- Deterministic local skill audits (`skillhub audit`)
- Per-skill trust decisions that gate MCP visibility (`skillhub trust`)
- Agent integration diagnostics (`skillhub doctor agents`)
- Minimal MCP stdio server with trust enforcement
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
