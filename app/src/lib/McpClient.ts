// Svelte-side wrapper around the 3 new Tauri commands. Components
// import `mcpClient` (or instantiate `McpClient`) instead of calling
// `invoke()` directly, so we can mock the transport in tests and
// centralize error handling in one place.
//
// Every method maps 1:1 to a Rust `#[tauri::command]` exposed by
// `app/src-tauri/src/commands.rs`:
//   runTool       -> mcp_run_tool
//   listTools     -> mcp_list_tools
//   getAuditEvent -> audit_event
//
// Errors from the operator socket surface as a thrown Error (the
// Tauri invoke channel rejects with a string error message from
// Rust). Components should `try { await client.runTool(...) } catch`
// and display the message; non-throwing result objects (e.g. an
// `ok: false` from a denied tool) are still returned as a value.

import { invoke } from "@tauri-apps/api/core";
import type { AuditEvent, McpRunResult } from "./types";

export class McpClient {
  /**
   * Run an MCP tool through the core. The core looks up the right
   * MCP server, runs the tool through Gate 3 (with operator
   * confirmation if the action class requires it), and returns the
   * result inline. The audit chain captures
   * `McpRunStarted` / `McpRunCompleted` for every call.
   */
  async runTool(domain: string, target: string, args: unknown): Promise<McpRunResult> {
    return await invoke<McpRunResult>("mcp_run_tool", { domain, target, args });
  }

  /**
   * List the tools available in a given MCP domain. For v1 the
   * catalog is hardcoded in `lib/toolCatalog.ts`; this Tauri command
   * returns an empty list (the Svelte side falls back to the bundled
   * catalog). A future sub-plan will wire the core to enumerate its
   * own tools via the existing `mcp.list_servers` machinery.
   */
  async listTools(domain: string): Promise<unknown[]> {
    return await invoke<unknown[]>("mcp_list_tools", { domain });
  }

  /**
   * Fetch a single audit event by id. Returns `null` if no event
   * matches the id (the core's `audit.query` with a filter that
   * matches nothing). The Tauri command serializes the event to
   * a generic JSON value; the cast happens on the consumer side.
   */
  async getAuditEvent(id: string): Promise<AuditEvent | null> {
    return await invoke<AuditEvent | null>("audit_event", { id });
  }

  /**
   * Page through the audit log. Returns the full QueryResponse from
   * the core: events + total_matched + page + page_size +
   * hash_chain_head + hash_chain_verified + query_ms. The Svelte
   * AuditLog component uses this to render the list + the "chain
   * verified at <hash>" badge.
   */
  async queryAudit(
    filter: unknown,
    page: number,
    pageSize: number,
  ): Promise<unknown> {
    return await invoke("audit_query", { filter, page, pageSize });
  }
}

/** Singleton for app-wide use. */
export const mcpClient = new McpClient();
