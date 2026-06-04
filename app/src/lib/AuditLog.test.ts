// Tests for the AuditLog list view. Asserts:
// - clicking a row calls onAuditClick with the event id
// - shows the chain-verified / chain-bad badge based on the response
// - prev/next buttons paginate
// - shows "no events yet" when events is empty
// - shows an error if mcpClient throws

import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockTauri } from "../test-utils";
mockTauri();
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import AuditLog from "./AuditLog.svelte";
import { invoke } from "@tauri-apps/api/core";

function makeEvent(id: string, kind = "mcp_run_completed", ts = "2026-06-03T12:00:00Z") {
  return { id, ts, kind, ok: true };
}

function makeResp(events: any[], extras: any = {}) {
  return {
    events,
    total_matched: events.length,
    page: 0,
    page_size: 50,
    hash_chain_head: "abcdef1234567890",
    hash_chain_verified: true,
    query_ms: 5,
    ...extras,
  };
}

describe("AuditLog", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows 'no events yet' when the log is empty", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(makeResp([]));
    const { findByTestId } = render(AuditLog, { onAuditClick: () => {} });
    await findByTestId("empty");
  });

  it("calls onAuditClick with the event id when a row is clicked", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(makeResp([makeEvent("evt-1"), makeEvent("evt-2")]));
    const onAuditClick = vi.fn();
    const { findAllByTestId } = render(AuditLog, { onAuditClick });
    const rows = await findAllByTestId("row");
    await fireEvent.click(rows[0]);
    expect(onAuditClick).toHaveBeenCalledWith("evt-1");
    await fireEvent.click(rows[1]);
    expect(onAuditClick).toHaveBeenCalledWith("evt-2");
  });

  it("shows the chain-verified badge when the chain is valid", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(makeResp([], { hash_chain_verified: true }));
    const { findByTestId } = render(AuditLog, { onAuditClick: () => {} });
    await findByTestId("chain-ok");
  });

  it("shows the chain-bad badge when the chain is not verified", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(makeResp([], { hash_chain_verified: false }));
    const { findByTestId } = render(AuditLog, { onAuditClick: () => {} });
    await findByTestId("chain-bad");
  });

  it("paginates via prev/next buttons", async () => {
    // First load: full page (50 events) so 'next' is enabled.
    vi.mocked(invoke).mockResolvedValueOnce(makeResp(Array.from({ length: 50 }, (_, i) => makeEvent(`e${i}`))));
    // After clicking next: empty page (so prev should be enabled, next disabled).
    vi.mocked(invoke).mockResolvedValueOnce(makeResp([]));
    const { findAllByTestId, getByTestId } = render(AuditLog, { onAuditClick: () => {} });
    const rows = await findAllByTestId("row");
    expect(rows.length).toBe(50);
    expect((getByTestId("prev") as HTMLButtonElement).disabled).toBe(true);
    expect((getByTestId("next") as HTMLButtonElement).disabled).toBe(false);
    await fireEvent.click(getByTestId("next"));
    await waitFor(() => {
      expect((getByTestId("prev") as HTMLButtonElement).disabled).toBe(false);
    });
  });

  it("shows an error message when mcpClient throws", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("socket disconnected"));
    const { findByTestId } = render(AuditLog, { onAuditClick: () => {} });
    await findByTestId("error");
  });
});
