// Tests for the ToolRunner middle pane. The component renders a list
// of tools for the selected domain; each has a Run button. The Run
// button calls mcpClient.runTool() and surfaces the result. We mock
// the underlying @tauri-apps invoke (which mcpClient wraps), same
// pattern as McpClient.test.ts.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { mockTauri } from "../test-utils";
mockTauri();
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import ToolRunner from "./ToolRunner.svelte";
import { TOOL_CATALOG } from "./toolCatalog";
import { invoke } from "@tauri-apps/api/core";

describe("ToolRunner", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("renders all tools for the selected domain", () => {
    const { getAllByRole } = render(ToolRunner, {
      domain: "ad",
      onRun: () => {},
    });
    const buttons = getAllByRole("button", { name: /Run/ });
    expect(buttons.length).toBe(TOOL_CATALOG.ad.length);
  });

  it("shows a hint message when no domain is selected", () => {
    const { getByText } = render(ToolRunner, { domain: null, onRun: () => {} });
    expect(getByText(/select a domain/i)).toBeTruthy();
  });

  it("calls invoke with mcp_run_tool and the right args when Run is clicked", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ ok: true, stdout: "ok" });
    let captured: any = null;
    const { getAllByRole } = render(ToolRunner, {
      domain: "osint",
      onRun: (r) => { captured = r; },
    });
    // The first osint tool is osint-whois
    const runButton = getAllByRole("button", { name: /Run/ })[0];
    await fireEvent.click(runButton);
    await waitFor(() => {
      expect(captured).toBeTruthy();
    });
    expect(invoke).toHaveBeenCalledWith("mcp_run_tool", {
      domain: "osint",
      target: "osint-whois",
      args: expect.anything(),
    });
    expect(captured.ok).toBe(true);
  });

  it("surfaces an error when args is not valid JSON", async () => {
    let captured: any = null;
    const { getAllByRole, getByTestId } = render(ToolRunner, {
      domain: "osint",
      onRun: (r) => { captured = r; },
    });
    // Type invalid JSON into the osint-whois textarea
    const ta = getByTestId("args-osint-whois") as HTMLTextAreaElement;
    ta.value = "not json {";
    await fireEvent.input(ta);
    const runButton = getAllByRole("button", { name: /Run/ })[0];
    await fireEvent.click(runButton);
    await waitFor(() => {
      expect(captured).toBeTruthy();
    });
    expect(captured.ok).toBe(false);
    expect(captured.error).toMatch(/not valid JSON/i);
  });

  it("surfaces the error from mcpClient when runTool throws", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("socket disconnected"));
    let captured: any = null;
    const { getAllByRole } = render(ToolRunner, {
      domain: "osint",
      onRun: (r) => { captured = r; },
    });
    const runButton = getAllByRole("button", { name: /Run/ })[0];
    await fireEvent.click(runButton);
    await waitFor(() => {
      expect(captured).toBeTruthy();
    });
    expect(captured.error).toBe("socket disconnected");
  });
});
