import { describe, it, expect, beforeEach } from "vitest";
import {
  state,
  workspace,
  selectDomain,
  setLastResult,
  openAuditDetail,
  closeAuditDetail,
} from "./state.svelte";

describe("app state", () => {
  it("starts disconnected", () => {
    expect(state.conn).toBe("disconnected");
  });

  it("transitions to connecting then connected", () => {
    state.conn = "connecting";
    expect(state.conn).toBe("connecting");
    state.conn = "connected";
    expect(state.conn).toBe("connected");
  });
});

describe("workspace state", () => {
  beforeEach(() => {
    workspace.selectedDomain = null;
    workspace.lastResult = null;
    workspace.auditDetailEventId = null;
  });

  it("starts with no domain, no result, no audit detail", () => {
    expect(workspace.selectedDomain).toBeNull();
    expect(workspace.lastResult).toBeNull();
    expect(workspace.auditDetailEventId).toBeNull();
  });

  it("selectDomain sets the domain and clears lastResult", () => {
    workspace.lastResult = { ok: true, stdout: "old" };
    selectDomain("ad");
    expect(workspace.selectedDomain).toBe("ad");
    expect(workspace.lastResult).toBeNull();
  });

  it("selectDomain(null) clears the selection", () => {
    selectDomain("osint");
    selectDomain(null);
    expect(workspace.selectedDomain).toBeNull();
  });

  it("setLastResult stores the result", () => {
    const r = { ok: true, stdout: "out" };
    setLastResult(r);
    // Svelte 5's $state deep-wraps objects, so reference equality
    // doesn't hold even when the content is the same — use deep
    // equality instead.
    expect(workspace.lastResult).toEqual(r);
  });

  it("openAuditDetail sets the event id", () => {
    openAuditDetail("evt-1");
    expect(workspace.auditDetailEventId).toBe("evt-1");
  });

  it("closeAuditDetail clears the event id", () => {
    openAuditDetail("evt-1");
    closeAuditDetail();
    expect(workspace.auditDetailEventId).toBeNull();
  });
});
