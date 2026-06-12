# Agent Skill Path Findings - Windows

Observed on Windows, user profile `C:\Users\viper`, during Task 1 of the
SkillHub v0.5 discovery engine plan.

## Claude Code

- User skills: `C:\Users\viper\.claude\skills`
  - Status: unconfirmed on this machine; the directory does not currently exist.
  - Registry behavior: include this conventional path; it may match nothing.
- Plugin skills: `C:\Users\viper\.claude\plugins\cache\<marketplace>\<plugin>\<version>\skills\<skill>\SKILL.md`
  - Status: confirmed.
  - Examples:
    - `C:\Users\viper\.claude\plugins\cache\claude-plugins-official\chrome-devtools-mcp\1.1.1\skills\chrome-devtools\SKILL.md`
    - `C:\Users\viper\.claude\plugins\cache\claude-plugins-official\superpowers\5.1.0\skills\test-driven-development\SKILL.md`
  - Directory depth from `.claude\plugins`: `cache/<marketplace>/<plugin>/<version>/skills/<skill>/SKILL.md`.
  - Implementation pattern: search under `.claude\plugins` for directories named `skills`; each `skills` directory is a plugin root. Ignore nested dependency matches such as `node_modules\...\ .agents\skills` by normal scan depth and manifest detection.
- Project skills: `<project>\.claude\skills`
  - Status: conventional project path; include via upward project lookup.

## Cursor

- Probed paths:
  - `C:\Users\viper\.cursor`
  - `C:\Users\viper\AppData\Roaming\Cursor`
  - `C:\Users\viper\AppData\Local\Cursor`
  - `C:\Users\viper\AppData\Roaming\Cursor\User\skills`
  - `C:\Users\viper\AppData\Roaming\Cursor\User\globalStorage`
- Status: unconfirmed on this machine; these paths do not currently exist and
  no Cursor `SKILL.md` was found before the probe was stopped.
- Registry behavior: include the conventional `~\.cursor\skills` path; it may
  match nothing on this machine.

## Codex

- User skills: `C:\Users\viper\.codex\skills\<skill>\SKILL.md`
  - Status: confirmed.
  - Examples:
    - `C:\Users\viper\.codex\skills\frontend-design`
    - `C:\Users\viper\.codex\skills\gstack`
- Plugin skills: `C:\Users\viper\.codex\plugins\cache\<marketplace>\<plugin>\<version>\skills\<skill>\SKILL.md`
  - Status: confirmed in this Codex install.
  - Examples:
    - `C:\Users\viper\.codex\plugins\cache\openai-bundled\browser\26.527.31326\skills\control-in-app-browser\SKILL.md`
    - `C:\Users\viper\.codex\plugins\cache\openai-curated\superpowers\c6ea566d\skills\test-driven-development\SKILL.md`
  - Directory depth from `.codex\plugins`: `cache/<marketplace>/<plugin>/<version>/skills/<skill>/SKILL.md`.
  - Implementation pattern: include the user skill root; plugin cache roots may
    be discovered with the same bounded `**\skills` search used for Claude if
    the registry implements Codex plugin discovery.

## Generic Agents

- Generic user skills: `C:\Users\viper\.agents\skills\<skill>\SKILL.md`
  - Status: confirmed.
  - Examples:
    - `C:\Users\viper\.agents\skills\academic-paper-reviewer`
    - `C:\Users\viper\.agents\skills\pdf-paper-reader`
- XDG-style roots: `C:\Users\viper\.config\*\skills`
  - Status: not confirmed in this probe.
  - Registry behavior: include on platforms where the path exists; it may match
    nothing.
