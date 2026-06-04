// Tests for the hardcoded tool catalog. The catalog mirrors the
// MCP crates' TOOLS constants; if you add a tool to a crate, add it
// here too. These tests catch drift early (a renamed tool, a
// forgotten destructive flag, a domain added/removed).
//
// The "expected" tool lists are duplicated here on purpose: they are
// the canonical source of truth for the Tauri UI. The Rust crate
// is the source of truth for the core. If the two ever drift, the
// Tauri UI will offer tools the core can't run (or vice versa) and
// the test will fail — that's the early warning we want.

import { describe, it, expect } from "vitest";
import { DOMAINS, TOOL_CATALOG, findTool, toolsForDomain } from "./toolCatalog";

const EXPECTED_AD = [
  "ad-impacket_psexec",
  "ad-impacket_wmiexec",
  "ad-impacket_secretsdump",
  "ad-impacket_kerberoast",
  "ad-impacket_asreproast",
].sort();

const EXPECTED_PACKETS = [
  "packets-tshark_read",
  "packets-pcap_export",
  "packets-tshark_capture",
  "packets-scapy_craft",
].sort();

const EXPECTED_OSINT = ["osint-whois", "osint-dig"].sort();

const EXPECTED_FLIPPER = [
  "flipper-list",
  "flipper-read",
  "flipper-write",
  "flipper-run",
].sort();

const EXPECTED_DETECT = ["detect-image", "detect-video", "detect-batch"].sort();

describe("toolCatalog", () => {
  it("DOMAINS contains all 6 expected domains in order", () => {
    expect(DOMAINS).toEqual(["osint", "packets", "ad", "flipper", "phish", "detect"]);
  });

  it("every domain has at least one tool", () => {
    for (const d of DOMAINS) {
      expect(TOOL_CATALOG[d].length).toBeGreaterThan(0);
    }
  });

  it("tool names within a domain are unique", () => {
    for (const d of DOMAINS) {
      const names = TOOL_CATALOG[d].map((t) => t.name);
      expect(new Set(names).size).toBe(names.length);
    }
  });

  it("tool names start with their domain prefix", () => {
    for (const d of DOMAINS) {
      for (const t of TOOL_CATALOG[d]) {
        expect(t.name.startsWith(`${d}-`)).toBe(true);
      }
    }
  });

  it.each([
    ["osint", EXPECTED_OSINT],
    ["packets", EXPECTED_PACKETS],
    ["ad", EXPECTED_AD],
    ["flipper", EXPECTED_FLIPPER],
    ["detect", EXPECTED_DETECT],
  ])("%s catalog matches the expected tool list", (domain, expected) => {
    const actual = TOOL_CATALOG[domain as keyof typeof TOOL_CATALOG]
      .map((t) => t.name)
      .sort();
    expect(actual).toEqual(expected);
  });

  it("phish catalog contains at least the evilginx + gophish tools", () => {
    const names = TOOL_CATALOG.phish.map((t) => t.name);
    for (const expected of [
      "phish-list",
      "phish-enable",
      "phish-disable",
      "phish-get_captures",
      "phish-lure_create",
      "phish-gophish_campaign_list",
      "phish-gophish_campaign_create",
      "phish-gophish_campaign_status",
      "phish-gophish_results",
    ]) {
      expect(names).toContain(expected);
    }
  });

  it("destructive flag is set on known destructive tools", () => {
    expect(findTool("ad-impacket_psexec")?.destructive).toBe(true);
    expect(findTool("phish-enable")?.destructive).toBe(true);
    expect(findTool("flipper-write")?.destructive).toBe(true);
    expect(findTool("osint-whois")?.destructive).toBe(false);
    expect(findTool("packets-tshark_read")?.destructive).toBe(false);
  });

  it("every tool has a non-empty description and argsSchema", () => {
    for (const d of DOMAINS) {
      for (const t of TOOL_CATALOG[d]) {
        expect(t.description.length).toBeGreaterThan(0);
        expect(t.argsSchema.length).toBeGreaterThan(0);
      }
    }
  });

  it("toolsForDomain returns the same array as TOOL_CATALOG[d]", () => {
    expect(toolsForDomain("osint")).toBe(TOOL_CATALOG.osint);
  });

  it("toolsForDomain returns an empty array for an unknown domain", () => {
    expect(toolsForDomain("not-a-domain" as any)).toEqual([]);
  });

  it("findTool returns undefined for an unknown tool", () => {
    expect(findTool("nope")).toBeUndefined();
  });
});
