//! Gate 3 (action-class confirmation) and Gate 4 (output sanitization) stubs.
//! See spec §4. Sub-plan 2 implements Gate 4 properly; sub-plan 4 implements Gate 3.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub domain: String,
    pub action_class: String,
    pub target: String,
    pub args: Value,
}

#[derive(Debug, Clone)]
pub struct SanitizedOutput {
    pub stdout: String,
    pub stderr: String,
    pub redacted_fields: Vec<String>,
    pub pi_detected: bool,
    pub pi_line_count: usize,
}

pub trait Gate3: Send + Sync {
    fn confirm(&self, req: &ActionRequest) -> Result<(), String>;
}

pub trait Gate4: Send + Sync {
    fn sanitize(&self, stdout: &str, stderr: &str) -> SanitizedOutput;
}

pub struct AllowAll;
impl Gate3 for AllowAll {
    fn confirm(&self, _req: &ActionRequest) -> Result<(), String> { Ok(()) }
}
impl Gate4 for AllowAll {
    fn sanitize(&self, stdout: &str, stderr: &str) -> SanitizedOutput {
        SanitizedOutput {
            stdout: stdout.into(),
            stderr: stderr.into(),
            redacted_fields: vec![],
            pi_detected: false,
            pi_line_count: 0,
        }
    }
}
