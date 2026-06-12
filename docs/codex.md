# Use SkillHub with Codex

Install SkillHub and scan your skills:

```bash
skillhub init
skillhub scan
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

If `skillhub` is not on PATH, use the absolute path to the binary in `command`.
