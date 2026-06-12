# MCP Tool Reference

SkillHub exposes a small read-first MCP API.

## `skillhub.search_skills`

Search indexed skills.

Input:

```json
{ "query": "web search" }
```

## `skillhub.list_skills`

List indexed skills.

Input:

```json
{}
```

## `skillhub.get_skill`

Return skill metadata and summary.

Input:

```json
{ "skill_id": "anysearch-skill" }
```

## `skillhub.get_skill_file`

Read a safe file from a skill directory.

Input:

```json
{ "skill_id": "anysearch-skill", "file": "SKILL.md" }
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
{ "skill_id": "anysearch-skill" }
```

## `skillhub.doctor_skill`

Run structured diagnostics for one skill.

Input:

```json
{ "skill_id": "anysearch-skill" }
```
