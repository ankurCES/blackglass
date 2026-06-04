// Rune-based reactive state.

import type { Domain, McpRunResult } from "./types";

export type ConnState = "disconnected" | "connecting" | "connected";

export type PendingRequest = {
  id: string;
  tool: string;
  domain: string;
  class: string;
  target: string;
  source: string;
  deadline_in_ms: number;
  received_at: number;
};

class AppState {
  conn: ConnState = $state("disconnected");
  pending: PendingRequest[] = $state([]);
  now: number = $state(Date.now());

  // Derived: a list of expired-by-now ids (used by the modal to fire
  // timeouts). The actual timeout event is fired once per id by the
  // modal's onMount; here we just expose "what's still live".
  live(): PendingRequest[] {
    return this.pending.filter((p) => p.deadline_in_ms > this.now - p.received_at);
  }

  enqueue(req: PendingRequest) { this.pending.push(req); }
  remove(id: string) { this.pending = this.pending.filter((p) => p.id !== id); }
}

export const state = new AppState();

// 100ms tick so the countdown in the modal updates smoothly.
if (typeof window !== "undefined") {
  setInterval(() => { state.now = Date.now(); }, 100);
}

// ---------------------------------------------------------------------------
// Workspace state (Phase 3.6). The 3-pane Tauri workspace lives here:
// - selectedDomain: which domain the user clicked in DomainRail
// - lastResult: the most recent McpRunResult from ToolRunner
// - auditDetailEventId: when non-null, the AuditDetail slide-out is open
//
// Components import `workspace` and the helper setters; they don't
// mutate the object directly because the helpers are easier to mock
// in tests and the API surface is small.
// ---------------------------------------------------------------------------

export const workspace = $state({
  selectedDomain: null as Domain | null,
  lastResult: null as McpRunResult | null,
  auditDetailEventId: null as string | null,
});

export function selectDomain(d: Domain | null) {
  workspace.selectedDomain = d;
  // Clear the result pane when the user switches domains — old
  // results from a different domain would be confusing.
  workspace.lastResult = null;
}

export function setLastResult(r: McpRunResult | null) {
  workspace.lastResult = r;
}

export function openAuditDetail(id: string) {
  workspace.auditDetailEventId = id;
}

export function closeAuditDetail() {
  workspace.auditDetailEventId = null;
}
