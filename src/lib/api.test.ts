// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import type { RuntimeInfo } from "@moor/types";
import { api } from "./api/client";
import { formatApiNetworkError } from "./api/errors";
import { resetRuntime } from "./api/runtime";

function runtime(port: number): RuntimeInfo {
  return {
    port,
    baseUrl: `http://127.0.0.1:${port}`,
    mcpUrl: `http://127.0.0.1:${port}/mcp`,
  };
}

function jsonResponse(body: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
    ...init,
  });
}

describe("web API client", () => {
  beforeEach(() => resetRuntime());

  afterEach(() => {
    vi.restoreAllMocks();
    resetRuntime();
  });

  it("retries a read once after a network failure", async () => {
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockRejectedValueOnce(new TypeError("Load failed"))
      .mockResolvedValueOnce(jsonResponse({ ok: true }));

    await expect(api<{ ok: boolean }>("/api/settings")).resolves.toEqual({ ok: true });

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "http://127.0.0.1:9223/api/settings",
      expect.objectContaining({
        credentials: "same-origin",
        headers: expect.objectContaining({ "Content-Type": "application/json" }),
      }),
    );
  });

  it("does not retry aborted requests", async () => {
    const controller = new AbortController();
    controller.abort();
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockRejectedValueOnce(new DOMException("This operation was aborted", "AbortError"));

    await expect(api("/api/settings", { signal: controller.signal })).rejects.toThrow(
      "This operation was aborted",
    );
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it.each(["POST", "PUT", "PATCH", "DELETE"])(
    "does not retry %s requests after a network failure",
    async (method) => {
      const fetchMock = vi
        .spyOn(globalThis, "fetch")
        .mockRejectedValueOnce(new TypeError("Load failed"));

      await expect(
        api("/api/settings", {
          method,
          body: method === "DELETE" ? undefined : JSON.stringify({ theme: "dark" }),
        }),
      ).rejects.toThrow("Unable to connect to the Moor server while requesting /api/settings");
      expect(fetchMock).toHaveBeenCalledTimes(1);
    },
  );

  it("does not retry an authentication failure", async () => {
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(
        jsonResponse({ error: { message: "Management authentication required" } }, { status: 401 }),
      );

    await expect(api("/api/settings")).rejects.toThrow("Management authentication required");
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("uses structured API error messages when available", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      jsonResponse(
        { error: { code: "VALIDATION_ERROR", message: "Invalid request timeout" } },
        { status: 400 },
      ),
    );

    await expect(api("/api/settings")).rejects.toThrow("Invalid request timeout");
  });

  it("falls back to a structured error code when the message is missing", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      jsonResponse({ error: { code: "NOT_FOUND" } }, { status: 404 }),
    );

    await expect(api("/api/profiles/missing")).rejects.toThrow("NOT_FOUND");
  });
});

describe("api error formatting", () => {
  it("adds server context to browser network failures", () => {
    expect(
      formatApiNetworkError("/api/settings", new TypeError("Load failed"), runtime(9225)),
    ).toBe(
      "Unable to connect to the Moor server while requesting /api/settings at http://127.0.0.1:9225. Check that the server is running and reachable. Original error: Load failed",
    );
  });
});
