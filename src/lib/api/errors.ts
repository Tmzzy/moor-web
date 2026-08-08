// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

import type { RuntimeInfo } from "@moor/types";
import { isRecord } from "@/lib/utils";

export function formatApiNetworkError(path: string, err: unknown, runtime?: RuntimeInfo): string {
  const detail = err instanceof Error ? err.message : String(err);
  const target = runtime ? ` at ${runtime.baseUrl}` : "";
  return `Unable to connect to the Moor server while requesting ${path}${target}. Check that the server is running and reachable. Original error: ${detail}`;
}

export function formatApiRetryError(
  path: string,
  runtime: RuntimeInfo,
  original: unknown,
  retryFailure: unknown,
): string {
  const originalDetail = original instanceof Error ? original.message : String(original);
  const retryDetail = retryFailure instanceof Error ? retryFailure.message : String(retryFailure);
  return `Unable to connect to the Moor server while requesting ${path} at ${runtime.baseUrl} after retrying. Original error: ${originalDetail}. Retry error: ${retryDetail}`;
}

export async function readApiError(resp: Response): Promise<string> {
  const parsed = (await resp.json().catch(() => null)) as unknown;
  if (isRecord(parsed)) {
    if (typeof parsed.error === "string") {
      return parsed.error;
    }
    if (isRecord(parsed.error)) {
      if (typeof parsed.error.message === "string" && parsed.error.message.length > 0) {
        return parsed.error.message;
      }
      if (typeof parsed.error.code === "string" && parsed.error.code.length > 0) {
        return parsed.error.code;
      }
    }
  }
  return resp.statusText || `API error: ${resp.status}`;
}

export async function parseApiResponse<T>(resp: Response): Promise<T> {
  if (!resp.ok) {
    throw new Error(await readApiError(resp));
  }
  return resp.json() as Promise<T>;
}

export function isAbortError(err: unknown, signal?: AbortSignal): boolean {
  return signal?.aborted === true || (isRecord(err) && err.name === "AbortError");
}

export function shouldRetryNetworkError(options?: RequestInit): boolean {
  const method = options?.method?.toUpperCase() ?? "GET";
  return method === "GET" || method === "HEAD";
}
