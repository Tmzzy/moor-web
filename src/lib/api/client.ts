// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

import type { RuntimeInfo } from "@moor/types";
import { createErrorWithCause } from "@/lib/utils";
import { notifyAuthenticationRequired } from "@/lib/auth-events";
import { getApiRuntime, refreshApiRuntime, buildApiUrl, buildApiHeaders } from "./runtime";
import {
  formatApiNetworkError,
  formatApiRetryError,
  readApiError,
  parseApiResponse,
  isAbortError,
  shouldRetryNetworkError,
} from "./errors";

export interface RequestOptions extends RequestInit {
  signal?: AbortSignal;
}

async function fetchWithRuntime(
  path: string,
  options: RequestOptions | undefined,
  runtime: RuntimeInfo,
): Promise<Response> {
  return fetch(buildApiUrl(runtime, path), {
    ...options,
    headers: buildApiHeaders(runtime, options?.headers),
    credentials: "same-origin",
    signal: options?.signal,
  });
}

async function retryWithFreshRuntime<T>(
  path: string,
  options: RequestOptions | undefined,
  originalError: unknown,
): Promise<T> {
  const runtime = await refreshApiRuntime();
  try {
    const retryResp = await fetchWithRuntime(path, options, runtime);
    if (retryResp.status === 401) notifyAuthenticationRequired();
    if (!retryResp.ok) {
      throw new Error(await readApiError(retryResp));
    }
    return retryResp.json() as Promise<T>;
  } catch (retryErr) {
    throw createErrorWithCause(
      formatApiRetryError(path, runtime, originalError, retryErr),
      originalError,
    );
  }
}

export async function api<T>(path: string, options?: RequestOptions): Promise<T> {
  const runtime = await getApiRuntime();
  let resp: Response;
  try {
    resp = await fetchWithRuntime(path, options, runtime);
  } catch (err) {
    if (isAbortError(err, options?.signal)) {
      throw err;
    }
    const networkError = createErrorWithCause(formatApiNetworkError(path, err, runtime), err);
    if (!shouldRetryNetworkError(options)) {
      throw networkError;
    }
    return retryWithFreshRuntime<T>(path, options, networkError);
  }
  if (resp.status === 401) notifyAuthenticationRequired();
  return parseApiResponse<T>(resp);
}

export async function apiPost<T>(path: string, body: unknown): Promise<T> {
  return api<T>(path, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function apiPut<T>(path: string, body: unknown): Promise<T> {
  return api<T>(path, {
    method: "PUT",
    body: JSON.stringify(body),
  });
}

export async function apiDelete<T>(path: string): Promise<T> {
  return api<T>(path, { method: "DELETE" });
}
