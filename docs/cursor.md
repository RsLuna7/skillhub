# Use SkillHub with Cursor

Add SkillHub as an MCP server using Cursor's MCP settings:

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

Restart Cursor or reload MCP servers.

Project-level `.skills` folders are scanned by SkillHub by default when you run `skillhub scan` from that project.

For first-time setup:

```bash
skillhub setup
```

Recommended instruction:

```bash
skillhub agent-instructions cursor
```
