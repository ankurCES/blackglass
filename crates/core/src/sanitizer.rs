//! Gate 4 prompt-injection detection and output sanitisation.

use std::sync::LazyLock;
use regex::RegexSet;

static PI_PATTERNS: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        // Direct AI role tokens at line start (strict: must be followed by colon, no intervening words)
        r"(?i)^\s*(AI|System|Assistant)\s*:",
        // Role tokens used as dialogue markers — USER: / HUMAN: / Human: (colon required; avoids "USER anonymous" in FTP)
        r"(?i)^\s*(USER|HUMAN|Human)\s*:",
        // Role-play / persona override at line start
        r"(?i)^\s*(pretend\s+you\s+are|act\s+as\b|you\s+must\s+now\b)",
        // Temporal instruction override
        r"(?i)^\s*(from\s+now\s+on|starting\s+now|henceforth)[,:\s]",
        // Explicit new-task injection
        r"(?i)^\s*new\s+task\s*:",
        // System-prompt injection phrases (anywhere in line)
        r"(?i)(the\s+following\s+(text|message|instructions?)\s+is\s+your\s+new\s+system\s+prompt|system\s+prompt\s*:)",
        // Constraint-removal at line start
        r"(?i)^\s*respond\s+only\s+as\b",
        // Common injection commands at line start
        r"(?i)^\s*(You\s+are\s+now|Disregard\s+(all\s+)?(prior|previous))[,:\s]",
        // Ignore-instructions anywhere in the line
        r"(?i)ignore\s+(all\s+)?(previous|prior)\s+instructions?",
        r"(?i)your\s+(new\s+)?(task|instruction|role|purpose)\s+is",
        // ChatML special tokens
        r"(?i)<\|im_(start|end)\|>",
        // GPT end-of-text token
        r"(?i)<\|endoftext\|>",
        // Llama/alpaca tokens
        r"(?i)\[/?INST\]",
        // Role headers on their own line (### System:)
        r"(?i)^\s*###\s*(System|Human|Assistant|User)\s*:?\s*$",
    ]).expect("PI pattern compilation failed")
});

/// Returns true if `line` looks like a prompt-injection attempt.
pub fn is_pi_line(line: &str) -> bool {
    PI_PATTERNS.is_match(line)
}

const BEGIN: &str = "=== BEGIN UNTRUSTED TOOL OUTPUT ===";
const END: &str = "=== END UNTRUSTED TOOL OUTPUT ===";
const REDACT: &str = "[REDACTED: prompt-injection-shaped content]";

/// Strip PI lines; returns (cleaned_text, count_removed, removed_lines).
pub fn strip_pi_lines(text: &str) -> (String, usize, Vec<String>) {
    let mut out = Vec::new();
    let mut count = 0usize;
    let mut removed = Vec::new();
    for line in text.lines() {
        if is_pi_line(line) {
            out.push(REDACT.to_string());
            removed.push(line.to_string());
            count += 1;
        } else {
            out.push(line.to_string());
        }
    }
    (out.join("\n"), count, removed)
}

/// Truncate `s` to at most `max_bytes` bytes at a UTF-8 character boundary.
pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut boundary = max_bytes;
    while !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &s[..boundary]
}

/// Wrap sanitised output in BEGIN/END markers.
pub fn wrap_output(cleaned: &str) -> String {
    format!("{}\n{}\n{}", BEGIN, cleaned, END)
}

use crate::gates::{Gate4, SanitizedOutput};
use std::path::PathBuf;

pub struct RealSanitizer {
    pub max_bytes: usize,
    pub evidence_dir: PathBuf,
}

impl RealSanitizer {
    pub fn new(max_bytes: usize, evidence_dir: PathBuf) -> Self {
        Self { max_bytes, evidence_dir }
    }
}

impl Gate4 for RealSanitizer {
    fn sanitize(&self, stdout: &str, stderr: &str) -> SanitizedOutput {
        let (cleaned, pi_count, removed) = strip_pi_lines(stdout);
        let truncated = truncate_utf8(&cleaned, self.max_bytes);
        let wrapped = wrap_output(truncated);
        SanitizedOutput {
            stdout: wrapped,
            stderr: stderr.to_string(),
            redacted_fields: removed,
            pi_detected: pi_count > 0,
            pi_line_count: pi_count,
        }
    }
}
