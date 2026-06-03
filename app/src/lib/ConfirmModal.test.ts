import { describe, it, expect, beforeEach, vi } from "vitest";
import { mockTauri } from "../test-utils";
mockTauri();
import { render, fireEvent } from "@testing-library/svelte";
import ConfirmModal from "./ConfirmModal.svelte";
import { state, type PendingRequest } from "./state.svelte";
import { invoke } from "@tauri-apps/api/core";

function makeRequest(overrides: Partial<PendingRequest> = {}): PendingRequest {
  return {
    id: "test-id-1",
    tool: "nmap_scan",
    domain: "recon",
    class: "destructive",
    target: "10.0.0.1",
    source: "ai-test",
    deadline_in_ms: 15_000,
    received_at: Date.now(),
    ...overrides,
  };
}

describe("ConfirmModal", () => {
  beforeEach(() => {
    state.pending = [];
    vi.mocked(invoke).mockClear();
  });

  it("renders the request details", () => {
    const req = makeRequest();
    const { getByText } = render(ConfirmModal, { request: req });
    expect(getByText("nmap_scan")).toBeTruthy();
    expect(getByText("10.0.0.1")).toBeTruthy();
    expect(getByText("destructive")).toBeTruthy();
  });

  it("Allow button sends confirm_resolve with 'allow' and removes from queue", async () => {
    const req = makeRequest();
    state.pending.push(req);
    const { getByText } = render(ConfirmModal, { request: req });
    await fireEvent.click(getByText("Allow"));
    expect(invoke).toHaveBeenCalledWith("confirm_resolve", {
      id: "test-id-1", decision: "allow",
    });
    expect(state.pending.length).toBe(0);
  });

  it("Deny button sends confirm_resolve with 'deny'", async () => {
    const req = makeRequest();
    const { getByText } = render(ConfirmModal, { request: req });
    await fireEvent.click(getByText("Deny"));
    expect(invoke).toHaveBeenCalledWith("confirm_resolve", {
      id: "test-id-1", decision: "deny",
    });
  });

  it("Allow & remember sends 'allow_and_remember'", async () => {
    const req = makeRequest();
    const { getByText } = render(ConfirmModal, { request: req });
    await fireEvent.click(getByText("Allow & remember"));
    expect(invoke).toHaveBeenCalledWith("confirm_resolve", {
      id: "test-id-1", decision: "allow_and_remember",
    });
  });
});
