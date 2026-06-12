# SkillHub

[![English](https://img.shields.io/badge/lang-English-blue.svg)](README.md)
[![简体中文](https://img.shields.io/badge/lang-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-red.svg)](README.zh-CN.md)

**给你的 AI 工具准备一个本地技能库。**

SkillHub 是一个轻量的 Rust CLI 和 MCP Server。它可以让 Codex、Claude Code、Cursor、OpenCode，以及其他支持 MCP 的工具，共用同一套本地 skills。

它会扫描你已有的 skill 目录，索引 `SKILL.md` 包，从 GitHub 安装 skill，并通过一个 MCP server 对外提供查询和读取能力。

## 快速开始

```bash
cargo install --git https://github.com/RsLuna7/skillhub
skillhub setup
```

把它接入任意 MCP 客户端：

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

尝试一次通用搜索：

```text
Use skillhub to find a template skill.
```

## 为什么需要它

Skills 正在变成一种可复用的本地能力包：里面可以包含说明、脚本、参考资料、模板和排错笔记。问题是，不同 AI 工具通常会把这些 skills 放在不同目录，也用不同方式发现它们。

SkillHub 给你一个统一的本地注册表：

- 找到其他工具已经安装的 skills。
- 从任何支持 MCP 的客户端搜索本地 skills。
- 按需读取 `SKILL.md`、`README.md` 和 `runtime.conf`。
- 查看推荐命令，但不直接执行。
- 用 dry-run 预览命令执行信息。
- 诊断缺失的 runtime 或环境变量。
- v0.2 默认安全：不会静默执行脚本。

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

## 示例

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

## 安装

从 GitHub 安装：

```bash
cargo install --git https://github.com/RsLuna7/skillhub
```

从源码安装：

```bash
git clone https://github.com/RsLuna7/skillhub.git
cd skillhub
cargo install --path .
```

也可以直接从 [Releases](https://github.com/RsLuna7/skillhub/releases) 下载二进制文件。

## 初始化

运行：

```bash
skillhub setup
```

这个命令会初始化 SkillHub、扫描已配置的 skill 目录、运行诊断，并打印 MCP 配置片段。它**不会**自动修改 Codex、Claude 或 Cursor 的配置文件。

打印可复制的工具使用指令：

```bash
skillhub agent-instructions
skillhub agent-instructions codex
```

## 接入 Codex

```bash
codex mcp add skillhub -- skillhub mcp
```

也可以手动把下面内容加入 `~/.codex/config.toml`：

```toml
[mcp_servers.skillhub]
command = "skillhub"
args = ["mcp"]
```

重启 Codex 后，可以做一次普通的 skill 查询：

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

`skillhub mcp` 会暴露这些工具：

```text
skillhub.search_skills
skillhub.list_skills
skillhub.get_skill
skillhub.get_skill_file
skillhub.get_skill_commands
skillhub.doctor_skill
```

## 默认位置

```text
Data:    ~/.skillhub
Index:   ~/.skillhub/index.sqlite
Config:  ~/.skillhub/config.toml
Install: ~/.agents/skills
```

默认扫描目录：

```text
~/.agents/skills
~/.claude/skills
~/.codex/skills
./.skills
```

添加新的 skill 目录：

```bash
skillhub config add-path /path/to/skills
skillhub scan
```

## 安全模型

SkillHub v0.2 以“先读取、再判断”为原则，默认保守。

- 它会索引和读取 skills。
- 它会返回推荐命令。
- `skillhub run` 只支持 dry-run。
- 它不会执行 skill 脚本。
- MCP 文件读取会阻止 `.env`、隐藏文件、绝对路径和路径穿越。
- 从 GitHub 安装 skill 时，如果目标目录已经存在，会拒绝覆盖。

## 当前状态

SkillHub 仍处于 alpha 阶段，适合本地试用和反馈。

已经实现：

- Rust 单二进制 CLI
- SQLite 本地索引，支持 FTS5 和 fallback 搜索
- 本地 skill 扫描和 capability 检测
- 从 GitHub 安装 skill
- `setup`、`search`、`show`、`doctor` 和 dry-run 命令预览
- 最小可用的 MCP stdio server
- MCP 文件读取安全检查

计划中：

- 官方 MCP SDK 实现
- 版本锁定和 lockfile
- 更强的 GitHub skill 供应链检查
- 可选的、带用户确认的命令执行

查看 [ROADMAP.md](ROADMAP.md)。

## 开发

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo check
```

## License

MIT
