// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

import { MCP_TIMEOUT_MS_MAX, MCP_TIMEOUT_MS_MIN } from "@moor/types";

export type SettingsPageLoadState =
  | { kind: "loading"; canRenderControls: false }
  | { kind: "error"; canRenderControls: false; message: string }
  | { kind: "ready"; canRenderControls: true };

export type TimeoutSecondsInputState =
  | { valid: true; milliseconds: number }
  | { valid: false; message: string };

const timeoutSecondsMin = MCP_TIMEOUT_MS_MIN / 1000;
const timeoutSecondsMax = MCP_TIMEOUT_MS_MAX / 1000;
const timeoutInputError = `Enter a whole number between ${timeoutSecondsMin} and ${timeoutSecondsMax}.`;

export function getSettingsPageLoadState({
  isLoading,
  isError,
  error,
}: {
  isLoading: boolean;
  isError: boolean;
  error: unknown;
}): SettingsPageLoadState {
  if (isLoading) {
    return { kind: "loading", canRenderControls: false };
  }
  if (isError) {
    return {
      kind: "error",
      canRenderControls: false,
      message: error instanceof Error ? error.message : "Failed to load settings",
    };
  }
  return { kind: "ready", canRenderControls: true };
}

export function parseTimeoutSecondsInput(value: string): TimeoutSecondsInputState {
  if (!/^\d+$/.test(value.trim())) {
    return { valid: false, message: timeoutInputError };
  }
  const seconds = Number(value);
  if (seconds < timeoutSecondsMin || seconds > timeoutSecondsMax) {
    return { valid: false, message: timeoutInputError };
  }
  return { valid: true, milliseconds: seconds * 1000 };
}
