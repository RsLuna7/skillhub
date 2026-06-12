use crate::config::AppConfig;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn install_github_skill(cfg: &AppConfig, source: &str) -> Result<PathBuf> {
    let (repo_slug, repo_url) = parse_github_source(source)?;
    let skill_id = repo_slug
        .split('/')
        .next_back()
        .unwrap_or("skill")
        .trim_end_matches(".git")
        .to_string();
    let target = cfg.install_dir_path()?.join(&skill_id);
    if target.exists() {
        bail!(
            "target already exists; refusing to overwrite: {}",
            target.display()
        );
    }
    let temp = tempfile::tempdir()?;
    let clone_dir = temp.path().join("repo");
    let status = Command::new("git")
        .args(["clone", "--depth", "1", &repo_url])
        .arg(&clone_dir)
        .status()
        .context("failed to run git clone")?;
    if !status.success() {
        bail!("git clone failed for {repo_url}");
    }
    let source_dir = choose_skill_root(&clone_dir)?;
    copy_dir(&source_dir, &target)?;
    Ok(target)
}

pub fn normalize_github_url(source: &str) -> Result<String> {
    Ok(parse_github_source(source)?.1)
}

fn parse_github_source(source: &str) -> Result<(String, String)> {
    if source.starts_with("https://github.com/") {
        let slug = source
            .trim_start_matches("https://github.com/")
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .to_string();
        return Ok((slug.clone(), format!("https://github.com/{slug}.git")));
    }
    if source.split('/').count() == 2 {
        return Ok((
            source.to_string(),
            format!("https://github.com/{source}.git"),
        ));
    }
    bail!("expected GitHub source like owner/repo or https://github.com/owner/repo");
}

fn choose_skill_root(repo: &Path) -> Result<PathBuf> {
    if repo.join("SKILL.md").exists() || repo.join("skill.yaml").exists() {
        return Ok(repo.to_path_buf());
    }
    let candidates = fs::read_dir(repo)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| path.join("SKILL.md").exists() || path.join("skill.yaml").exists())
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [only] => Ok(only.clone()),
        [] => bail!("repository does not contain a root skill; no SKILL.md or skill.yaml found"),
        many => bail!(
            "repository contains multiple skill candidates; v1 requires a root skill or single candidate: {}",
            many.iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            copy_dir(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}
