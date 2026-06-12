//! Dep-free typed parser for SKILL.md / skill.yaml frontmatter.
//! Handles the simple `key: value` frontmatter SkillHub actually uses; surfaces
//! malformed frontmatter as an error instead of silently missing a skill.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillManifest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ManifestError {
    NoFrontmatter,
    Unterminated,
}

impl SkillManifest {
    /// A folder is a skill only if its manifest carries at least a name.
    pub fn is_valid(&self) -> bool {
        self.name
            .as_ref()
            .is_some_and(|name| !name.trim().is_empty())
    }
}

/// Parse a leading `---`-delimited frontmatter block. Returns an error when the
/// block is absent or unterminated so callers can surface malformed manifests.
pub fn parse_frontmatter(content: &str) -> Result<SkillManifest, ManifestError> {
    let trimmed = content.strip_prefix('\u{feff}').unwrap_or(content);
    if !trimmed.starts_with("---") {
        return Err(ManifestError::NoFrontmatter);
    }
    let after = &trimmed[3..];
    let after = after
        .strip_prefix('\n')
        .or_else(|| after.strip_prefix("\r\n"))
        .unwrap_or(after);
    let end = after.find("\n---").ok_or(ManifestError::Unterminated)?;
    let block = &after[..end];

    let mut manifest = SkillManifest::default();
    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        match key.trim() {
            "name" => manifest.name = Some(value),
            "description" => manifest.description = Some(value),
            _ => {}
        }
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_description() {
        let md = "---\nname: Hello\ndescription: \"Does a thing\"\n---\n# Hello\n";
        let m = parse_frontmatter(md).unwrap();
        assert_eq!(m.name.as_deref(), Some("Hello"));
        assert_eq!(m.description.as_deref(), Some("Does a thing"));
    }

    #[test]
    fn no_frontmatter_is_error() {
        assert_eq!(
            parse_frontmatter("# Just a heading\n"),
            Err(ManifestError::NoFrontmatter)
        );
    }

    #[test]
    fn unterminated_frontmatter_is_error() {
        assert_eq!(
            parse_frontmatter("---\nname: X\n"),
            Err(ManifestError::Unterminated)
        );
    }

    #[test]
    fn is_skill_requires_name() {
        let with = SkillManifest {
            name: Some("X".into()),
            description: None,
        };
        let without = SkillManifest::default();
        assert!(with.is_valid());
        assert!(!without.is_valid());
    }
}
