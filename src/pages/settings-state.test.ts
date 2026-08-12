// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

import { describe, expect, it } from "vite-plus/test";
import { getSettingsPageLoadState, parseTimeoutSecondsInput } from "./settings-state";
import {
  createDefaultSettings,
  MCP_REQUEST_TIMEOUT_MS_DEFAULT,
  MCP_SERVER_START_TIMEOUT_MS_DEFAULT,
  MCP_TIMEOUT_MS_DEFAULT,
  MCP_TIMEOUT_MS_MAX,
  MCP_TIMEOUT_MS_MIN,
} from "@moor/types";

describe("settings page state helpers", () => {
  it("blocks settings controls when the settings query failed", () => {
    expect(
      getSettingsPageLoadState({
        isLoading: false,
        isError: true,
        error: new Error("Unable to connect"),
      }),
    ).toEqual({
      kind: "error",
      canRenderControls: false,
      message: "Unable to connect",
    });
  });

  it("accepts only integer timeout seconds inside the supported range", () => {
    expect(parseTimeoutSecondsInput("5")).toEqual({
      valid: true,
      milliseconds: MCP_TIMEOUT_MS_MIN,
    });
    expect(parseTimeoutSecondsInput("300")).toEqual({
      valid: true,
      milliseconds: MCP_TIMEOUT_MS_MAX,
    });
    expect(parseTimeoutSecondsInput("")).toEqual({
      valid: false,
      message: "Enter a whole number between 5 and 300.",
    });
    expect(parseTimeoutSecondsInput("5.7")).toEqual({
      valid: false,
      message: "Enter a whole number between 5 and 300.",
    });
    expect(parseTimeoutSecondsInput("4")).toEqual({
      valid: false,
      message: "Enter a whole number between 5 and 300.",
    });
    expect(parseTimeoutSecondsInput("301")).toEqual({
      valid: false,
      message: "Enter a whole number between 5 and 300.",
    });
  });

  it("keeps request and server-start timeout defaults independent", () => {
    const defaults = createDefaultSettings();

    expect(defaults.advanced).toMatchObject({
      mcpRequestTimeoutMs: MCP_REQUEST_TIMEOUT_MS_DEFAULT,
      mcpServerStartTimeoutMs: MCP_SERVER_START_TIMEOUT_MS_DEFAULT,
    });
    expect(MCP_TIMEOUT_MS_DEFAULT).toBe(MCP_REQUEST_TIMEOUT_MS_DEFAULT);
  });
});
