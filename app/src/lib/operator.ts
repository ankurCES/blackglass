import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export type OperatorEvent = {
  kind: "confirm.request";
  raw: { params: { id: string; tool: string; domain: string; class: string; target: string; source: string; deadline_in_ms: number } };
};

export async function listenForOperatorEvents(
  handler: (e: OperatorEvent) => void
): Promise<UnlistenFn> {
  return await listen<OperatorEvent>("operator-event", (e) => handler(e.payload));
}

export async function sendResolve(id: string, decision: "allow" | "allow_and_remember" | "deny"): Promise<void> {
  await invoke("confirm_resolve", { id, decision });
}
