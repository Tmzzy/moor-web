import { describe, expect, it } from "vite-plus/test";
import { routes } from "./api-routes";

describe("api routes", () => {
  it("builds management authentication and profile token routes", () => {
    expect(routes.auth.session()).toBe("/api/auth/session");
    expect(routes.auth.login()).toBe("/api/auth/login");
    expect(routes.auth.logout()).toBe("/api/auth/logout");
    expect(routes.profiles.mcpToken("work/profile")).toBe("/api/profiles/work%2Fprofile/mcp-token");
    expect(routes.profiles.rotateMcpToken("work/profile")).toBe(
      "/api/profiles/work%2Fprofile/mcp-token/rotate",
    );
  });

  it("encodes dynamic path segments and tool query parameters", () => {
    expect(routes.servers.detail("server/1")).toBe("/api/servers/server%2F1");
    expect(routes.servers.profiles("server/1")).toBe("/api/servers/server%2F1/profiles");
    expect(routes.profiles.updateServer("profile&1", "server/1")).toBe(
      "/api/profiles/profile%261/servers/server%2F1",
    );
    expect(routes.servers.tools("server/1", "profile&mode=all")).toBe(
      "/api/servers/server%2F1/tools?profile_id=profile%26mode%3Dall",
    );
  });

  it("builds encoded logs query parameters from an object", () => {
    expect(
      routes.logs.list({
        server_id: "server/1",
        tool_name: "read&write",
        from: "2026-01-01T00:00:00Z",
      }),
    ).toBe("/api/logs?server_id=server%2F1&tool_name=read%26write&from=2026-01-01T00%3A00%3A00Z");
  });
});
