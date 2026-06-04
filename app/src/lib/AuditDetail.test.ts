// Tests for the AuditDetail far-right slide-out. Asserts:
// - renders nothing when no eventId is set
// - loads and renders the event when an id is set
// - calls onClose when the X is clicked

import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockTauri } from "../test-utils";
mockTauri();
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import AuditDetail from "./AuditDetail.svelte";
import { invoke } from "@tauri-apps/api/core";

describe("AuditDetail", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows 'not found' when the core returns null", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(null);
    const { findByText } = render(AuditDetail, { eventId: "missing", onClose: () => {} });
    await findByText(/not found/i);
  });

  it("renders nothing when no event id is set", () => {
    const { container } = render(AuditDetail, { eventId: null, onClose: () => {} });
    // Svelte 5 + jsdom leaves a comment/text node where the
    // {#if eventId} block was, so we can't assert firstChild is null
    // — but the <aside.audit-detail> should not exist.
    expect(container.querySelector("aside.audit-detail")).toBeNull();
  });

  it("loads and renders the event when an id is set", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      id: "e1",
      ts: "2026-06-03T12:00:00Z",
      kind: "mcp_run_completed",
      ok: true,
      ms: 1234,
    });
    const { findByText } = render(AuditDetail, { eventId: "e1", onClose: () => {} });
    await findByText(/mcp_run_completed/);
    await findByText(/1234/);
  });

  it("calls onClose when the X is clicked", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ id: "e1", kind: "x" });
    const onClose = vi.fn();
    const { findByText } = render(AuditDetail, { eventId: "e1", onClose });
    const closeBtn = await findByText("×");
    await fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
