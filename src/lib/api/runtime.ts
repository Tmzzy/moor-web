// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

import type { RuntimeInfo } from "@moor/types";

const FALLBACK_BASE_URL = "http://127.0.0.1:9223";

function runtimeBaseUrl(): string {
  const configured = import.meta.env.VITE_MOOR_API_URL?.trim();
  if (configured) return configured.replace(/\/$/, "");
  if (typeof window !== "undefined" && window.location.origin !== "null") {
    return window.location.origin;
  }
  return FALLBACK_BASE_URL;
}

function resolveRuntime(): RuntimeInfo {
  const baseUrl = runtimeBaseUrl();
  const url = new URL(baseUrl);
  const port = Number(url.port || (url.protocol === "https:" ? 443 : 80));
  return { port, baseUrl, mcpUrl: `${baseUrl}/mcp` };
}

let runtimeInfo: RuntimeInfo | null = null;

export function resetRuntime(): void {
  runtimeInfo = null;
}

export async function getApiRuntime(): Promise<RuntimeInfo> {
  runtimeInfo ??= resolveRuntime();
  return runtimeInfo;
}

export async function refreshApiRuntime(): Promise<RuntimeInfo> {
  resetRuntime();
  return getApiRuntime();
}

export function buildApiUrl(runtime: RuntimeInfo, path: string): string {
  return `${runtime.baseUrl}${path}`;
}

export function buildApiHeaders(_runtime: RuntimeInfo, extra?: HeadersInit): HeadersInit {
  const extraHeaders = new Headers(extra);
  const headers: Record<string, string> = {};
  if (!extraHeaders.has("Content-Type")) {
    headers["Content-Type"] = "application/json";
  }
  extraHeaders.forEach((value, key) => {
    headers[key] = value;
  });
  return headers;
}
