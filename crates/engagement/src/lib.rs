//! Engagement model + Gate 2 (target allowlist). See spec §1.3.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngagementError {
    #[error("invalid CIDR: {0}")]
    BadCidr(String),
    #[error("invalid IP: {0}")]
    BadIp(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind { Ip, Cidr, Hostname }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub value: String,
    pub kind: TargetKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Engagement {
    pub id: String,
    pub name: String,
    pub scope_start: String,
    pub scope_end: String,
    pub targets: Vec<Target>,
}

impl Engagement {
    pub fn new(id: &str, name: &str, start: &str, end: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            scope_start: start.into(),
            scope_end: end.into(),
            targets: vec![],
        }
    }
    pub fn add_target(&mut self, t: Target) {
        self.targets.push(t);
    }

    /// Returns true iff `value` matches at least one target.
    pub fn allows(&self, value: &str) -> bool {
        for t in &self.targets {
            match t.kind {
                TargetKind::Ip => {
                    if t.value == value {
                        return true;
                    }
                }
                TargetKind::Cidr => {
                    if let (Ok(net), Ok(ip)) = (
                        t.value.parse::<ipnetwork::IpNetwork>(),
                        value.parse::<std::net::IpAddr>(),
                    ) {
                        if net.contains(ip) {
                            return true;
                        }
                    }
                }
                TargetKind::Hostname => {
                    if t.value.eq_ignore_ascii_case(value) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
