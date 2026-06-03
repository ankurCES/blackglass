//! Policy: which action classes a session is allowed to invoke.
//!
//! Sub-plan 3 introduces a simplified policy model. Gate 1 (policy) and
//! Gate 2 (engagement) are folded into the chokepoint's own logic; this
//! module only owns the action-class allowlist that drives Gate 1.
//! See spec §4 (Gate 1) and the chokepoint `evaluate` function.

use serde::{Deserialize, Serialize};

/// The classification of an action. Gate 1 (policy) checks the requested
/// class against the active `Policy`'s allowlist. Destructive actions
/// additionally require a Gate 3 operator confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionClass {
    ReadOnly,
    Destructive,
}

/// The session policy. The default allows both classes; the analyst
/// `Profile` is what narrows the allowlist for actual sessions.
#[derive(Debug, Clone)]
pub struct Policy {
    pub action_classes: Vec<ActionClass>,
}

impl Policy {
    /// Returns `true` if `class` is permitted by this policy.
    pub fn allows(&self, class: ActionClass) -> bool {
        self.action_classes.contains(&class)
    }
}

impl Default for Policy {
    fn default() -> Self {
        // Default policy is permissive at the policy level; the per-session
        // `Profile` (set by the operator) is what restricts classes.
        Self { action_classes: vec![ActionClass::ReadOnly, ActionClass::Destructive] }
    }
}
