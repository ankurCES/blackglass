import { describe, it, expect } from "vitest";
import { state } from "./state.svelte";

describe("app state", () => {
  it("starts disconnected", () => {
    expect(state.conn).toBe("disconnected");
  });

  it("transitions to connecting then connected", () => {
    state.conn = "connecting";
    expect(state.conn).toBe("connecting");
    state.conn = "connected";
    expect(state.conn).toBe("connected");
  });
});
