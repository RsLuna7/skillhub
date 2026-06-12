use std::path::{Path, PathBuf};

pub mod priority {
    pub const USER_CONFIG: u8 = 50;
    pub const PROJECT: u8 = 40;
    pub const USER_GLOBAL: u8 = 20;
    pub const PLUGIN: u8 = 10;
}

/// A directory to scan, tagged with the agent it belongs to and a priority used
/// to resolve duplicate skill ids (higher wins).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredRoot {
    pub agent: String,
    pub path: PathBuf,
    pub priority: u8,
}

impl DiscoveredRoot {
    fn new(agent: &str, path: PathBuf, priority: u8) -> Self {
        Self {
            agent: agent.to_string(),
            path,
            priority,
        }
    }
}

/// A provider knows which directories on this machine belong to one agent.
/// It performs pure path logic only; it never scans or parses skills.
pub trait SkillProvider {
    fn name(&self) -> &str;
    fn roots(&self, home: &Path, cwd: &Path) -> Vec<DiscoveredRoot>;
}

pub struct ClaudeProvider;

impl SkillProvider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }

    fn roots(&self, home: &Path, cwd: &Path) -> Vec<DiscoveredRoot> {
        let mut roots = vec![DiscoveredRoot::new(
            "claude",
            home.join(".claude").join("skills"),
            priority::USER_GLOBAL,
        )];
        roots.extend(plugin_skill_dirs(
            &home.join(".claude").join("plugins"),
            "claude",
        ));
        if let Some(dir) = find_up(cwd, &[".claude", "skills"]) {
            roots.push(DiscoveredRoot::new("claude", dir, priority::PROJECT));
        }
        roots
    }
}

pub struct CursorProvider;

impl SkillProvider for CursorProvider {
    fn name(&self) -> &str {
        "cursor"
    }

    fn roots(&self, home: &Path, _cwd: &Path) -> Vec<DiscoveredRoot> {
        vec![DiscoveredRoot::new(
            "cursor",
            home.join(".cursor").join("skills"),
            priority::USER_GLOBAL,
        )]
    }
}

pub struct CodexProvider;

impl SkillProvider for CodexProvider {
    fn name(&self) -> &str {
        "codex"
    }

    fn roots(&self, home: &Path, _cwd: &Path) -> Vec<DiscoveredRoot> {
        let mut roots = vec![DiscoveredRoot::new(
            "codex",
            home.join(".codex").join("skills"),
            priority::USER_GLOBAL,
        )];
        roots.extend(plugin_skill_dirs(
            &home.join(".codex").join("plugins"),
            "codex",
        ));
        roots
    }
}

pub struct GenericProvider;

impl SkillProvider for GenericProvider {
    fn name(&self) -> &str {
        "generic"
    }

    fn roots(&self, home: &Path, _cwd: &Path) -> Vec<DiscoveredRoot> {
        let mut roots = vec![DiscoveredRoot::new(
            "generic",
            home.join(".agents").join("skills"),
            priority::USER_GLOBAL,
        )];
        let config = home.join(".config");
        if let Ok(entries) = std::fs::read_dir(&config) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    roots.push(DiscoveredRoot::new(
                        "generic",
                        entry.path().join("skills"),
                        priority::USER_GLOBAL,
                    ));
                }
            }
        }
        roots
    }
}

pub struct ProjectProvider;

impl SkillProvider for ProjectProvider {
    fn name(&self) -> &str {
        "project"
    }

    fn roots(&self, _home: &Path, cwd: &Path) -> Vec<DiscoveredRoot> {
        let mut roots = Vec::new();
        if let Some(dir) = find_up(cwd, &[".skills"]) {
            roots.push(DiscoveredRoot::new("project", dir, priority::PROJECT));
        }
        roots
    }
}

fn find_up(start: &Path, segments: &[&str]) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let candidate = join_segments(dir, segments);
        if candidate.exists() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    Some(join_segments(start, segments))
}

fn join_segments(root: &Path, segments: &[&str]) -> PathBuf {
    segments
        .iter()
        .fold(root.to_path_buf(), |path, seg| path.join(seg))
}

fn plugin_skill_dirs(plugins_dir: &Path, agent: &str) -> Vec<DiscoveredRoot> {
    use walkdir::WalkDir;

    if !plugins_dir.exists() {
        return vec![DiscoveredRoot::new(
            agent,
            plugins_dir.join("cache"),
            priority::PLUGIN,
        )];
    }

    let roots = WalkDir::new(plugins_dir)
        .max_depth(6)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir() && entry.file_name() == "skills")
        .map(|entry| DiscoveredRoot::new(agent, entry.into_path(), priority::PLUGIN))
        .collect::<Vec<_>>();

    if roots.is_empty() {
        vec![DiscoveredRoot::new(
            agent,
            plugins_dir.join("cache"),
            priority::PLUGIN,
        )]
    } else {
        roots
    }
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn SkillProvider>>,
}

impl ProviderRegistry {
    pub fn with_builtins() -> Self {
        Self {
            providers: vec![
                Box::new(ClaudeProvider),
                Box::new(CursorProvider),
                Box::new(CodexProvider),
                Box::new(GenericProvider),
                Box::new(ProjectProvider),
            ],
        }
    }

    /// All roots from every provider plus user-configured extras, de-duplicated
    /// by path, keeping the highest priority on collision.
    pub fn all_roots(&self, home: &Path, cwd: &Path, extra: &[PathBuf]) -> Vec<DiscoveredRoot> {
        let mut all = Vec::new();
        for path in extra {
            all.push(DiscoveredRoot::new(
                "user-config",
                path.clone(),
                priority::USER_CONFIG,
            ));
        }
        for provider in &self.providers {
            all.extend(provider.roots(home, cwd));
        }
        dedupe_by_path(all)
    }
}

fn dedupe_by_path(roots: Vec<DiscoveredRoot>) -> Vec<DiscoveredRoot> {
    let mut out: Vec<DiscoveredRoot> = Vec::new();
    for root in roots {
        if let Some(existing) = out.iter_mut().find(|r| r.path == root.path) {
            if root.priority > existing.priority {
                existing.priority = root.priority;
                existing.agent = root.agent;
            }
        } else {
            out.push(root);
        }
    }
    out
}
