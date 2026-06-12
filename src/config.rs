use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub data_dir: String,
    pub default_install_dir: String,
    pub scan_roots: Vec<String>,
    pub mcp: McpConfig,
    #[serde(skip)]
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub max_file_chars: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        let data_dir =
            std::env::var("SKILLHUB_DATA_DIR").unwrap_or_else(|_| "~/.skillhub".to_string());
        let default_install_dir = std::env::var("SKILLHUB_INSTALL_DIR")
            .unwrap_or_else(|_| "~/.agents/skills".to_string());
        Self {
            data_dir,
            default_install_dir,
            scan_roots: vec![
                std::env::var("SKILLHUB_INSTALL_DIR")
                    .unwrap_or_else(|_| "~/.agents/skills".to_string()),
                "~/.claude/skills".to_string(),
                "~/.codex/skills".to_string(),
                ".skills".to_string(),
            ],
            mcp: McpConfig {
                max_file_chars: 12_000,
            },
            config_path: default_config_path(),
        }
    }
}

impl AppConfig {
    pub fn load_or_init() -> Result<Self> {
        if !default_config_path().exists() {
            return init_config();
        }
        Self::load()
    }

    pub fn load() -> Result<Self> {
        let config_path = default_config_path();
        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?;
        let mut cfg: AppConfig = toml::from_str(&content)?;
        cfg.config_path = config_path;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.config_path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn data_dir_path(&self) -> Result<PathBuf> {
        expand_path(&self.data_dir)
    }

    pub fn install_dir_path(&self) -> Result<PathBuf> {
        expand_path(&self.default_install_dir)
    }

    pub fn index_path(&self) -> PathBuf {
        self.data_dir_path()
            .unwrap_or_else(|_| default_data_dir())
            .join("index.sqlite")
    }

    pub fn expanded_scan_roots(&self) -> Vec<PathBuf> {
        self.scan_roots
            .iter()
            .filter_map(|root| expand_path(root).ok())
            .collect()
    }
}

pub fn init_config() -> Result<AppConfig> {
    let cfg = AppConfig::default();
    fs::create_dir_all(cfg.data_dir_path()?)?;
    fs::create_dir_all(cfg.install_dir_path()?)?;
    cfg.save()?;
    Ok(cfg)
}

pub fn expand_path(input: &str) -> Result<PathBuf> {
    if input == "~" {
        return Ok(home_dir());
    }
    if let Some(rest) = input.strip_prefix("~/") {
        return Ok(home_dir().join(rest));
    }
    if let Some(rest) = input.strip_prefix("~\\") {
        return Ok(home_dir().join(rest));
    }
    let path = Path::new(input);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn default_config_path() -> PathBuf {
    default_data_dir().join("config.toml")
}

fn default_data_dir() -> PathBuf {
    if let Ok(path) = std::env::var("SKILLHUB_DATA_DIR") {
        return PathBuf::from(path);
    }
    home_dir().join(".skillhub")
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}
