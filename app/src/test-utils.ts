import { vi } from "vitest";

export function mockTauri() {
  vi.mock("@tauri-apps/api/event", () => ({
    listen: vi.fn().mockResolvedValue(() => {}),
  }));
  vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn().mockResolvedValue(undefined),
  }));
}
