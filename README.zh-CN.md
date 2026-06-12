# SkillHub

[![English](https://img.shields.io/badge/lang-English-blue.svg)](README.md)
[![简体中文](https://img.shields.io/badge/lang-%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87-red.svg)](README.zh-CN.md)

**AI skills 的本地控制平面。**

SkillHub 是一个轻量的 Rust CLI 和 MCP Server。它可以让 Codex、Claude Code、Cursor、OpenCode，以及其他支持 MCP 的工具，共用同一套本地 skills，并由你来决定这些工具能看到哪些 skills。

它会扫描你已有的 skill 目录，索引 `SKILL.md` 包，从 GitHub 安装 skill，用确定性的本地规则审计 skill，并通过一个带信任控制的 MCP server 对外提供查询和读取能力。

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

而当 agents 能发现 skills 之后，总得有人来决定哪些 skills 值得信任。这个决定应该留在你的机器上，而不是交给云服务。

SkillHub 给你一个统一的本地注册表和控制平面：

- 找到其他工具已经安装的 skills。
- 从任何支持 MCP 的客户端搜索本地 skills。
- 按需读取 `SKILL.md`、`README.md` 和 `runtime.conf`。
- 用确定性的本地静态规则审计 skills。
- 按机器允许或屏蔽 skills；被屏蔽的 skills 不会出现在 MCP 中。
- 查看推荐命令，但不直接执行。
- 用 dry-run 预览命令执行信息。
- 诊断缺失的 runtime 或环境变量，并检查各 agent 的接入状态。
- 默认安全：不会静默执行脚本。

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

$ skillhub audit template-skill
template-skill: pass (0 findings, rules v2)

$ skillhub trust block risky-skill --reason "review pending"
risky-skill: blocked
Hidden from MCP clients until unblocked.

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
skillhub audit [skill-id] [--json]
skillhub trust list [--json]
skillhub trust allow <skill-id>
skillhub trust block <skill-id> [--reason <text>]
skillhub trust reset <skill-id>
skillhub connect codex|claude|cursor [--dry-run]
skillhub demo
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

`skillhub mcp` 会暴露这些工具：

```text
skillhub.search_skills
skillhub.list_skills
skillhub.get_skill
skillhub.get_skill_file
skillhub.get_skill_commands
skillhub.doctor_skill
```

MCP 的返回结果会遵守你的本地信任决定：被屏蔽的 skills 不会出现在 `list_skills` / `search_skills` 中，其余工具也会拒绝访问它们。`get_skill` 会包含 `trust`、`audit` 和 `visibility` 元数据，让 agent 知道一个 skill 被审查到什么程度。

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

## 信任与审计

SkillHub v0.3 在注册表之上增加了一个本地控制平面。

用确定性的离线规则审计某个 skill（或全部）：

```bash
skillhub audit
skillhub audit some-skill --json
```

审计会标记这些模式：把下载内容直接管道进 shell、破坏性删除、动态代码执行、硬编码密钥、`sudo`、混淆代码，以及明文 HTTP 端点。v2 规则带上下文感知以降低误报：markdown 正文永远不扫描（*提到* `rm -rf` 的文档不是攻击）、markdown 代码块按降级严重度扫描、`rm` 只有在指向 `/`、`~` 这类系统路径时才算严重。结果保存在本地 SQLite 索引中，并出现在 `skillhub show` 和 MCP `get_skill` 里。

决定 agents 能看到什么：

```bash
skillhub trust list
skillhub trust allow some-skill
skillhub trust block some-skill --reason "review pending"
skillhub trust reset some-skill
```

被屏蔽的 skills 在 CLI 中对你仍然可见，但对 MCP 客户端隐藏并拒绝访问。一切都在本地：没有云服务，没有遥测，审计过程不联网。

## 安全模型

SkillHub 以“先读取、再判断”为原则，默认保守。

- 它会索引和读取 skills。
- 它会返回推荐命令。
- `skillhub run` 只支持 dry-run。
- 它不会执行 skill 脚本。
- 审计是本地、确定性的，且完全离线。
- 被屏蔽的 skills 对 MCP 客户端不可见。
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
- 确定性的本地 skill 审计（`skillhub audit`）
- 按 skill 的信任决定，并控制 MCP 可见性（`skillhub trust`）
- agent 接入诊断（`skillhub doctor agents`）
- 带信任控制的最小 MCP stdio server
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
