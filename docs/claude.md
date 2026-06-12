# Use SkillHub with Claude

Add SkillHub as an MCP stdio server in your Claude MCP configuration:

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

Restart Claude after editing the config.

If `skillhub` is not on PATH, use the absolute path to the binary.

SkillHub scans `~/.claude/skills` by default, so Claude-specific skills can be discovered by other agents through SkillHub.

Run setup first:

```bash
skillhub setup
```

Recommended instruction:

```bash
skillhub agent-instructions claude
```
