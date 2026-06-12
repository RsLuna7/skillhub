# SkillHub v0.5 Discovery Engine — Design

**Status:** Approved (design phase)
**Date:** 2026-06-12
**Baseline:** `feat/v0.4-adoption` (v0.4.0)

## Problem

SkillHub v0.4 discovers skills by scanning a hardcoded list of `~/`-relative
directories at `skillhub scan` time, then serving a static SQLite snapshot over
MCP. This has three concrete failures:

1. **Breadth.** Skills installed outside the four hardcoded roots are invisible:
   Claude Code *plugin* skills (not in `~/.claude/skills`), Cursor's own skill
   directory, XDG `~/.config/*/skills`, anything on Windows/macOS that isn't
   `~/`-relative. The paths are also home-relative only — not cross-OS.
2. **Freshness.** The index is a snapshot. A skill installed after the last
   `scan` is invisible until the user manually re-scans. No live update.
3. **Detection + provenance.** A folder counts as a skill if it has
   `SKILL.md`/`skill.yaml` *or* a README merely containing "skill"/"agent"/
   "scripts" — both false-positive and false-negative prone. The same skill
   found in multiple roots is resolved by "last scan wins" (non-deterministic),
   and there is no record of which agent a skill came from.

## Goal

Make discovery **broad, fresh, and deterministic** without abandoning the
project's character: single binary, fully offline, deterministic, **no resident
daemon**.

## Non-Goals (deferred to v0.6)

- `skillhub scan --deep` (whole-`$HOME` glob for `**/SKILL.md`).
- `skillhub roots list/add/remove` management commands.
- `connect <agent>` also registering that agent's skill dir as a scan root.
- `skillhub watch` (file-watching mode).
- Rendering the README demo GIF.

## Key Decisions (from brainstorming)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Run model | **No daemon; lazy incremental** | Preserve single-binary/offline ethos; "fresh on access" not "real-time". |
| Discovery strategy | **Provider registry default** (deep scan deferred) | Covers ~95% of cases, fast and explainable, no noise. |
| Detection strictness | **Tighten + layer** | Canonical = `SKILL.md`/`skill.yaml` with valid frontmatter; fuzzy README rule demoted to a `--deep`-only weak signal (v0.6). |
| Duplicate handling | **Priority dedup + provenance** | Deterministic winner; record every source — this is what makes it a real cross-agent control plane. |
| v0.5 scope | **Core engine only** | One focused spec: providers + cross-OS paths, provenance + priority dedup, lazy incremental, tightened detection. |

## Architecture

Upgrade the hardcoded `scan_roots: Vec<String>` into a **provider layer** while
keeping the main pipeline shape — scan → SQLite index → MCP query — unchanged
(backward compatible).

```
ProviderRegistry      (new) one SkillProvider per agent; expands its skill-dir
       │                    conventions per-OS; tags each root with agent+priority
       │ roots() + source labels
       ▼
scan engine           (changed) iterates (provider, root) not bare paths;
 (src/scan.rs)                   carries provenance; mtime-incremental; tightened detection
       │ Skill { ..., source_agent, source_root }   ← new fields
       ▼
SQLite index          (changed) provenance columns; priority dedup;
  (src/db.rs)                    skill_sources table records all duplicate locations
       │
       ▼
MCP / CLI             (changed) list/get return source_agent;
                                MCP lazily validates root mtime → re-scan only stale roots
```

**Separation of concerns:** A `SkillProvider` only answers "which directories on
*this* machine belong to *this* agent" — pure, per-OS path logic, unit-testable
in isolation. It never scans or parses. The scan engine knows nothing about
specific agents; it consumes a `Vec<DiscoveredRoot>`. The two communicate only
through that interface.

## Components

### `src/providers.rs` (new)

```rust
pub struct DiscoveredRoot {
    pub agent: String,   // "claude" | "cursor" | "codex" | "generic" | "project" | "user-config"
    pub path: PathBuf,
    pub priority: u8,    // higher wins on duplicate; see Dedup
}

pub trait SkillProvider {
    fn name(&self) -> &str;
    fn roots(&self, home: &Path, cwd: &Path) -> Vec<DiscoveredRoot>;
}
```

Built-in providers, each expanding paths **per-OS** (Windows
`%APPDATA%`/`%USERPROFILE%`, macOS `~/Library/...`, Linux XDG `~/.config`):

- `ClaudeProvider` — `~/.claude/skills`, Claude Code **plugin** skill dirs
  (plugin cache, e.g. `~/.claude/plugins/*/skills`), project `.claude/skills`.
- `CursorProvider` — Cursor's skill directory.
- `CodexProvider` — `~/.codex/skills`.
- `GenericProvider` — `~/.agents/skills`, XDG `~/.config/*/skills`.
- `ProjectProvider` — walk up from `cwd` for `.skills` / `.claude/skills`.

> **Open item for implementation:** the exact on-disk path of Claude Code plugin
> skills and the Cursor skill directory must be verified against a real install
> during Task 1, not assumed. The plan will include a verification step.

`ProviderRegistry::all_roots(home, cwd)` aggregates every provider's roots,
de-duplicates by `(agent, canonical path)`, preserves each root's priority, and
appends user-configured roots from `config.toml` as a highest-priority provider.

### `src/config.rs` (changed)

`scan_roots` stays for **user-supplied extra roots only** (backward compatible:
existing configs still honored, injected as `user-config` priority). The four
hardcoded defaults move into the provider registry. Existing v0.4 configs that
list the old defaults keep working (they just become explicit user roots).

### `src/db.rs` (changed)

- `skills` table: add `source_agent TEXT`, `source_root TEXT` (additive).
- New `skill_sources(skill_id, agent, root, priority)` — every location a skill
  id was found, for the "also present in…" view.
- New `scan_state(root, last_scanned_sig, last_scanned_at)` — incremental scan
  bookkeeping (see Freshness).
- Migration: additive `ALTER TABLE`/`CREATE TABLE IF NOT EXISTS`, following the
  existing v0.2→v0.3 in-place migration pattern.

## Data Model & Priority Dedup

During a scan, candidates for each skill id are collected as
`(skill, root, priority)`:

- The **highest-priority** candidate becomes the live row in `skills`.
- Priority convention: `user-config (50) > project (40) > user-global (20) > plugin (10)`,
  where `user-global` covers the claude/cursor/codex/generic agent directories
  and `plugin` covers plugin-bundled skills. An explicit root in `config.toml`
  is the strongest signal (the user asked for it by hand), so it wins outright.
- Ties broken by root mtime (newer wins) → fully deterministic, killing
  "last scan wins".
- **All** candidates are written to `skill_sources`, so `list`/`get` can report
  "`gstack`: live copy is the project one; also present in Claude global and Codex".

## Freshness: Lazy Incremental Re-scan (no daemon)

`scan_state` stores a cheap signature per root (top-level dir mtime + mtimes of
its immediate skill subdirs — **stat only, never a full walk**).

- **CLI `scan`** — compares each root's signature; unchanged roots are skipped
  entirely, only changed roots are re-walked. `scan --force` does a full re-scan.
- **MCP startup + every `list_skills`/`search_skills`** — runs the cheap
  signature check; if a root is stale, synchronously re-scans **that one root**
  (millisecond-scale) before returning. Zero user perception, never stale, no
  resident process.
- **Degradation guard:** the freshness check must remain O(roots) stat calls.
  It must never trigger a full tree walk on every MCP call. This boundary is
  called out explicitly in the implementation plan.

## Data Flow (end to end)

```
scan → ProviderRegistry::all_roots() → for each CHANGED root: walk (depth 3,
       skip .git/node_modules/target/.venv) → tightened detection + YAML parse →
       collect candidates by id → priority dedup →
       write skills (winner) + skill_sources (all) + update scan_state

mcp list/search → cheap mtime signature check → re-scan stale root(s) if needed →
                  query skills (with trust filter) → return results incl. source_agent
```

## Tightened Detection

- Default: folder has `SKILL.md` **or** `skill.yaml` **and** parses a valid
  frontmatter (`name` at minimum). The "README contains keyword" rule is removed
  (demoted to a v0.6 `--deep` weak signal).
- Replace the hand-rolled `find("---")` frontmatter parsing with real YAML
  parsing into a typed `SkillManifest` — sturdier and can surface parse errors.
- **Known backward-compat cost:** skills currently indexed *only* via the fuzzy
  README rule will drop out. The plan includes a before/after diff step that
  lists every "no longer recognized under v0.5" entry for the maintainer to
  confirm they are noise, not real skills.

## Backward Compatibility

- CLI surface: additive only. No removed commands or flags.
- JSON shapes (`audit --json`, `show --json`, MCP `get_skill`/`list_skills`):
  additive only — new `source_agent` field; nothing removed or renamed.
- Existing `config.toml` with the old `scan_roots` keeps working.
- DB migration is in-place and additive.

## Testing Strategy

- `providers.rs` — per-provider path unit tests (given home/cwd → expected
  roots), exercised **cross-OS** via injected home/cwd rather than reading the
  real environment.
- Dedup — same skill id in multiple roots: assert highest priority wins,
  `skill_sources` records all, mtime tiebreak is deterministic.
- Incremental — scan once → touch one root → assert only that root re-walked,
  others skipped.
- Detection — fuzzy-rule fixture now rejected; valid `SKILL.md` still accepted;
  malformed frontmatter surfaces an error rather than a silent miss.
- Compatibility — `audit --json` / `show --json` / MCP `get_skill` shapes are
  additive (gain `source_agent`); no field removed.
- Quality gates unchanged: `cargo fmt --check`, `cargo clippy --all-targets
  --all-features -- -D warnings`, `cargo test`.

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| Plugin/Cursor skill paths assumed wrong | Task 1 verifies against a real install before coding the provider. |
| Freshness check degrades into full walk | Explicit O(roots)-stat boundary + a test asserting no full walk on MCP calls. |
| Tightened detection drops real skills | Before/after diff step surfaces every dropped id for maintainer sign-off. |
| Cross-OS path bugs | Inject home/cwd in tests; cover Windows/macOS/Linux expansion. |
