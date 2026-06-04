// Shared types for the domain workspace. The Rust side has its own
// versions of these in `app/src-tauri/src/commands.rs` (McpRunRequest
// / McpRunResponse); keep them in sync when one side changes.

export type Domain = "osint" | "packets" | "ad" | "flipper" | "phish" | "detect";

export interface Tool {
  /** Dotted tool name, e.g. "ad-impacket_psexec". Unique within a domain. */
  name: string;
  /** One-line human description shown in the ToolRunner list. */
  description: string;
  /** JSON-formatted example args shown as a placeholder hint in the textarea. */
  argsSchema: string;
  /** True if the tool is destructive and requires Gate 3 confirmation. */
  destructive: boolean;
}

/** Result of a single mcp_run_tool invocation. Mirrors `McpRunResponse` in Rust. */
export interface McpRunResult {
  ok: boolean;
  stdout?: string;
  stderr?: string;
  /** Audit event id for this run; opens the audit-detail right rail. */
  audit_event_id?: string;
  error?: string;
}

/** A single audit event returned by the core. */
export interface AuditEvent {
  id: string;
  ts: string;
  kind: string;
  [key: string]: unknown;
}
