// Rune-based reactive state. Sub-plan 3 v1: just the connection status.
// Pending-queue and modal state land in Task 14.

export type ConnState = "disconnected" | "connecting" | "connected";

class AppState {
  conn: ConnState = $state("disconnected");
}

export const state = new AppState();
