// Hardcoded tool catalog. Mirrors the `*_TOOLS` constants and the
// `Tool::new(...)` lists in the MCP crates. When you add a new tool
// to a crate, update both the crate's tools.rs AND this file. (A
// future sub-plan will replace this with a runtime-fetched catalog
// via the core's mcp.list_tools method; for v1 the catalog lives in
// the bundle so the Tauri UI is useful even when the core is down.)
//
// Destructive flag: any tool marked `destructive: true` flows through
// Gate 3 in the chokepoint and pops a confirm modal in the Tauri UI
// before it runs. Non-destructive tools run inline.

import type { Domain, Tool } from "./types";

export const TOOL_CATALOG: Record<Domain, Tool[]> = {
  osint: [
    {
      name: "osint-whois",
      description: "WHOIS lookup for a domain or IP",
      argsSchema: '{ "target": "example.com" }',
      destructive: false,
    },
    {
      name: "osint-dig",
      description: "DNS dig lookup",
      argsSchema: '{ "target": "example.com", "type": "A" }',
      destructive: false,
    },
  ],
  packets: [
    {
      name: "packets-tshark_read",
      description: "Read a pcap with tshark",
      argsSchema: '{ "pcap": "/path/to.pcap", "filter": "tcp.port==80" }',
      destructive: false,
    },
    {
      name: "packets-pcap_export",
      description: "Copy/filter a pcap to a destination path",
      argsSchema: '{ "pcap": "/path/to.pcap", "dst": "/path/to/out.pcap", "filter": "http" }',
      destructive: false,
    },
    {
      name: "packets-tshark_capture",
      description: "Live capture with tshark",
      argsSchema: '{ "iface": "eth0", "duration_s": 30 }',
      destructive: false,
    },
    {
      name: "packets-scapy_craft",
      description: "Craft a packet with scapy",
      argsSchema: '{ "layers": "IP(dst=\\"10.0.0.5\\")/TCP()" }',
      destructive: false,
    },
  ],
  ad: [
    {
      name: "ad-impacket_psexec",
      description: "impacket psexec (run cmd on remote Windows)",
      argsSchema: '{ "target": "10.0.0.5", "user": "admin", "cmd": "whoami" }',
      destructive: true,
    },
    {
      name: "ad-impacket_wmiexec",
      description: "impacket wmiexec (run cmd over WMI)",
      argsSchema: '{ "target": "10.0.0.5", "user": "admin", "cmd": "whoami" }',
      destructive: true,
    },
    {
      name: "ad-impacket_secretsdump",
      description: "impacket secretsdump (dump SAM/LSA secrets)",
      argsSchema: '{ "target": "10.0.0.5", "user": "admin" }',
      destructive: true,
    },
    {
      name: "ad-impacket_kerberoast",
      description: "impacket GetUserSPNs (Kerberoast)",
      argsSchema: '{ "domain": "EXAMPLE.COM", "user": "admin", "password": "..." }',
      destructive: true,
    },
    {
      name: "ad-impacket_asreproast",
      description: "impacket GetNPUsers (AS-REP roast)",
      argsSchema: '{ "domain": "EXAMPLE.COM" }',
      destructive: true,
    },
  ],
  flipper: [
    {
      name: "flipper-list",
      description: "List files on the Flipper",
      argsSchema: "{}",
      destructive: false,
    },
    {
      name: "flipper-read",
      description: "Read a file from the Flipper",
      argsSchema: '{ "path": "/any/sub.txt" }',
      destructive: false,
    },
    {
      name: "flipper-write",
      description: "Write a file to the Flipper",
      argsSchema: '{ "path": "/any/sub.txt", "content": "..." }',
      destructive: true,
    },
    {
      name: "flipper-run",
      description: "Run a saved Flipper payload (subghz/ibutton/etc.)",
      argsSchema: '{ "path": "/any/sub.fre" }',
      destructive: true,
    },
  ],
  phish: [
    {
      name: "phish-list",
      description: "List loaded evilginx phishlets",
      argsSchema: "{}",
      destructive: false,
    },
    {
      name: "phish-enable",
      description: "Enable an evilginx phishlet",
      argsSchema: '{ "name": "o365" }',
      destructive: true,
    },
    {
      name: "phish-disable",
      description: "Disable an evilginx phishlet",
      argsSchema: '{ "name": "o365" }',
      destructive: false,
    },
    {
      name: "phish-get_captures",
      description: "List captured credentials from evilginx",
      argsSchema: "{}",
      destructive: false,
    },
    {
      name: "phish-lure_create",
      description: "Create an evilginx lure URL",
      argsSchema: '{ "phishlet": "o365", "redirect_url": "https://example.com" }',
      destructive: true,
    },
    {
      name: "phish-gophish_campaign_list",
      description: "List gophish campaigns",
      argsSchema: "{}",
      destructive: false,
    },
    {
      name: "phish-gophish_campaign_create",
      description: "Create a gophish campaign",
      argsSchema: '{ "name": "...", "template": "...", "url": "https://..." }',
      destructive: true,
    },
    {
      name: "phish-gophish_campaign_status",
      description: "Get the status of a gophish campaign",
      argsSchema: '{ "id": 1 }',
      destructive: false,
    },
    {
      name: "phish-gophish_results",
      description: "Get the results of a completed gophish campaign",
      argsSchema: '{ "id": 1 }',
      destructive: false,
    },
  ],
  detect: [
    {
      name: "detect-image",
      description: "Analyze an image for deepfake indicators",
      argsSchema: '{ "image_path": "/path/to.jpg" }',
      destructive: false,
    },
    {
      name: "detect-video",
      description: "Analyze a video for deepfake indicators",
      argsSchema: '{ "video_path": "/path/to.mp4" }',
      destructive: false,
    },
    {
      name: "detect-batch",
      description: "Batch-analyze a directory of media files",
      argsSchema: '{ "dir": "/path/to/dir" }',
      destructive: false,
    },
  ],
};

export const DOMAINS: Domain[] = [
  "osint",
  "packets",
  "ad",
  "flipper",
  "phish",
  "detect",
];

export function toolsForDomain(domain: Domain): Tool[] {
  return TOOL_CATALOG[domain] ?? [];
}

export function findTool(name: string): Tool | undefined {
  for (const domain of DOMAINS) {
    const t = toolsForDomain(domain).find((t) => t.name === name);
    if (t) return t;
  }
  return undefined;
}
