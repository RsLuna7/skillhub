use anyhow::{Result, bail};
use std::path::{Component, Path, PathBuf};

pub fn safe_skill_file_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let rel_path = Path::new(relative);
    if rel_path.is_absolute() {
        bail!("absolute paths are not allowed");
    }
    if relative == ".env" || relative.contains(".env.") && !relative.ends_with(".env.example") {
        bail!("secret-like env files are not readable");
    }
    for component in rel_path.components() {
        match component {
            Component::ParentDir => bail!("path traversal is not allowed"),
            Component::Normal(name)
                if name.to_string_lossy().starts_with('.') && name != ".env.example" =>
            {
                bail!("hidden files are not readable")
            }
            _ => {}
        }
    }
    let joined = root.join(rel_path);
    let root_canon = root.canonicalize()?;
    let file_canon = joined.canonicalize()?;
    if !file_canon.starts_with(root_canon) {
        bail!("requested file is outside the skill directory");
    }
    Ok(file_canon)
}
