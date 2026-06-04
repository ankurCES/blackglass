// Tests for the McpClient Svelte wrapper. We mock the Tauri invoke
// channel using the existing test-utils helper and assert the right
// command names + arg shapes are sent for each call. We also assert
// that the result types are surfaced as-is (no silent mapping).

import { describe, it, expect, beforeEach, vi } from "vitest";
import { mockTauri } from "../test-utils";
mockTauri();
import { McpClient } from "./McpClient";
import { invoke } from "@tauri-apps/api/core";

describe("McpClient", () => {
  let client: McpClient;

  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    client = new McpClient();
  });

  it("runTool calls mcp_run_tool with the right args", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      ok: true,
      stdout: "hello",
      stderr: "",
      audit_event_id: "evt-1",
    });
    const result = await client.runTool("ad", "ad-impacket_psexec", { target: "10.0.0.5" });
    expect(invoke).toHaveBeenCalledWith("mcp_run_tool", {
      domain: "ad",
      target: "ad-impacket_psexec",
      args: { target: "10.0.0.5" },
    });
    expect(result.ok).toBe(true);
    expect(result.stdout).toBe("hello");
  });

  it("runTool surfaces the error message on failure", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      ok: false,
      error: "gate denied",
    });
    const result = await client.runTool("ad", "ad-impacket_psexec", {});
    expect(result.ok).toBe(false);
    expect(result.error).toBe("gate denied");
  });

  it("runTool throws on transport error", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("socket disconnected"));
    await expect(
      client.runTool("ad", "ad-impacket_psexec", {})
    ).rejects.toThrow("socket disconnected");
  });

  it("getAuditEvent calls audit_event with the right id", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ kind: "mcp_run_completed", ok: true });
    const evt = await client.getAuditEvent("evt-1");
    expect(invoke).toHaveBeenCalledWith("audit_event", { id: "evt-1" });
    expect(evt).not.toBeNull();
    expect((evt as any).kind).toBe("mcp_run_completed");
  });

  it("getAuditEvent returns null when the core returns null", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(null);
    const evt = await client.getAuditEvent("missing");
    expect(evt).toBeNull();
  });

  it("listTools calls mcp_list_tools with the right domain", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);
    const tools = await client.listTools("osint");
    expect(invoke).toHaveBeenCalledWith("mcp_list_tools", { domain: "osint" });
    expect(Array.isArray(tools)).toBe(true);
  });
});
