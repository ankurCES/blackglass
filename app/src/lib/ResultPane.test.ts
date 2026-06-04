// Tests for the ResultPane right-middle pane. Asserts:
// - placeholder when no result
// - stdout renders when present
// - onAuditClick fires with the audit_event_id
// - error renders in red when ok:false

import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import ResultPane from "./ResultPane.svelte";

describe("ResultPane", () => {
  it("shows a placeholder when there is no result", () => {
    const { getByText } = render(ResultPane, { result: null, onAuditClick: () => {} });
    expect(getByText(/no result yet/i)).toBeTruthy();
  });

  it("shows stdout when present", () => {
    const { getByText } = render(ResultPane, {
      result: { ok: true, stdout: "hello world", stderr: "", audit_event_id: "e1" },
      onAuditClick: () => {},
    });
    expect(getByText("hello world")).toBeTruthy();
  });

  it("calls onAuditClick with the audit_event_id when the audit link is clicked", async () => {
    const onAuditClick = vi.fn();
    const { getByText } = render(ResultPane, {
      result: { ok: true, stdout: "", stderr: "", audit_event_id: "evt-42" },
      onAuditClick,
    });
    await fireEvent.click(getByText("evt-42"));
    expect(onAuditClick).toHaveBeenCalledWith("evt-42");
  });

  it("shows the error in red when ok is false", () => {
    const { container, getByTestId } = render(ResultPane, {
      result: { ok: false, error: "gate denied" },
      onAuditClick: () => {},
    });
    const errEl = getByTestId("error");
    expect(errEl).toBeTruthy();
    expect(errEl.textContent).toContain("gate denied");
    expect(container.querySelector(".error")).toBeTruthy();
  });
});
