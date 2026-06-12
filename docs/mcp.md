# MCP Tool Reference

SkillHub exposes a small read-first MCP API.

All tools respect local trust decisions made with `skillhub trust`: blocked skills are excluded from `list_skills` and `search_skills`, and the per-skill tools return an error (`blocked by local trust policy`) for them.

## `skillhub.search_skills`

Search indexed skills.

Input:

```json
{ "query": "template" }
```

## `skillhub.list_skills`

List indexed skills.

Input:

```json
{}
```

## `skillhub.get_skill`

Return skill metadata, summary, files, commands, source, detected capabilities, and next actions, plus trust/audit/visibility metadata:

- `trust`: `trusted`, `untrusted` (default), or `blocked`
- `trust_reason`: optional reason recorded with `skillhub trust block`
- `audit`: latest audit result (`status`, `findings`, `rules_version`, `audited_at`) or `null` if never audited
- `visibility`: `visible` or `blocked` (what MCP clients get)

Input:

```json
{ "skill_id": "template-skill" }
```

## `skillhub.get_skill_file`

Read a safe file from a skill directory.

Input:

```json
{ "skill_id": "template-skill", "file": "SKILL.md" }
```

Blocked:

- absolute paths
- path traversal
- `.env`
- hidden files except `.env.example`

## `skillhub.get_skill_commands`

Return recommended commands without executing them.

Input:

```json
{ "skill_id": "template-skill" }
```

## `skillhub.doctor_skill`

Run structured diagnostics for one skill.

Input:

```json
{ "skill_id": "template-skill" }
```

## Execution Boundary

SkillHub does not expose command execution over MCP. Use `skillhub.get_skill_commands` to inspect commands only. The CLI command `skillhub run <skill-id> <command-index> --dry-run` previews a command without executing it.
