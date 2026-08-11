import { describe, expect, it } from "vite-plus/test";

import { safeDestination } from "./Login";

describe("login destination", () => {
  it("keeps an internal destination", () => {
    expect(safeDestination({ from: "/servers/server-1?tab=tools" })).toBe(
      "/servers/server-1?tab=tools",
    );
  });

  it.each([
    undefined,
    null,
    { from: "https://evil.example" },
    { from: "//evil.example" },
    { from: "/login" },
  ])("falls back to the dashboard for an unsafe destination", (state) => {
    expect(safeDestination(state)).toBe("/");
  });
});
