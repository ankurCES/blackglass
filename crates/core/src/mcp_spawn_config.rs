//! Config loader for the MCP supervisor. Reads
//! `~/.config/blackglass/mcp-servers.toml` into a typed struct.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("path {0} does not exist")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerSpec {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
}

fn default_startup_timeout_ms() -> u64 {
    30_000
}
fn default_max_restarts() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpSpawnConfig {
    #[serde(default)]
    pub servers: Vec<McpServerSpec>,
}

impl McpSpawnConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }
        let s = fs::read_to_string(path)?;
        Ok(toml::from_str(&s)?)
    }
}
