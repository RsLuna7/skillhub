# Use SkillHub with Codex

Install SkillHub and run setup:

```bash
skillhub setup
```

Add it to Codex:

```bash
codex mcp add skillhub -- skillhub mcp
```

Or edit `~/.codex/config.toml`:

```toml
[mcp_servers.skillhub]
command = "skillhub"
args = ["mcp"]
```

Restart Codex, then ask:

```text
Use skillhub to search my local skills for web search.
```

Recommended instruction for Codex:

```bash
skillhub agent-instructions codex
```

If `skillhub` is not on PATH, use the absolute path to the binary in `command`.
