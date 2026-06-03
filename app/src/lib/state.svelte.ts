// Rune-based reactive state.

export type ConnState = "disconnected" | "connecting" | "connected";

export type PendingRequest = {
  id: string;
  tool: string;
  domain: string;
  class: string;
  target: string;
  source: string;
  deadline_in_ms: number;
  received_at: number;
};

class AppState {
  conn: ConnState = $state("disconnected");
  pending: PendingRequest[] = $state([]);
  now: number = $state(Date.now());

  // Derived: a list of expired-by-now ids (used by the modal to fire
  // timeouts). The actual timeout event is fired once per id by the
  // modal's onMount; here we just expose "what's still live".
  live(): PendingRequest[] {
    return this.pending.filter((p) => p.deadline_in_ms > this.now - p.received_at);
  }

  enqueue(req: PendingRequest) { this.pending.push(req); }
  remove(id: string) { this.pending = this.pending.filter((p) => p.id !== id); }
}

export const state = new AppState();

// 100ms tick so the countdown in the modal updates smoothly.
if (typeof window !== "undefined") {
  setInterval(() => { state.now = Date.now(); }, 100);
}
