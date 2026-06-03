//! Profile loader. See ADR 0003.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unknown tier: {0}")]
    UnknownTier(String),
    #[error("no profile name")]
    MissingName,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Analyst,
    Operator,
    Redteam,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub tier: Tier,
    pub allowed_domains: Vec<String>,
    pub allowed_action_classes: Vec<String>,
}

impl Profile {
    pub fn parse(s: &str) -> Result<Self, ProfileError> {
        // First pass: pull the raw tier string so we can emit UnknownTier for
        // present-but-invalid values. The derived Deserialize would otherwise
        // surface those as a generic toml error and UnknownTier would be dead.
        let raw: toml::Value = toml::from_str(s)?;
        if let Some(t) = raw.get("tier").and_then(|v| v.as_str()) {
            match t {
                "analyst" | "operator" | "redteam" => {}
                other => return Err(ProfileError::UnknownTier(other.to_string())),
            }
        }
        // Second pass: full deserialize. Tier is now guaranteed valid if present.
        let p: Profile = serde::Deserialize::deserialize(raw)?;
        if p.name.is_empty() {
            return Err(ProfileError::MissingName);
        }
        Ok(p)
    }

    pub fn analyst_default() -> Self {
        Self {
            name: "analyst".into(),
            tier: Tier::Analyst,
            allowed_domains: vec!["core".into(), "osint".into(), "packets".into(), "audit".into()],
            allowed_action_classes: vec!["read_only".into()],
        }
    }

    pub fn allows_domain(&self, domain: &str) -> bool {
        self.allowed_domains.iter().any(|d| d == domain)
    }
    pub fn allows_action_class(&self, cls: &str) -> bool {
        self.allowed_action_classes.iter().any(|c| c == cls)
    }
}
