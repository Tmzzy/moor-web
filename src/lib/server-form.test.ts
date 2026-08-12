import { describe, expect, it } from "vite-plus/test";
import {
  argsToArrayOrNull,
  argsToArrayOrUndefined,
  entriesToRecordOrNull,
  entriesToRecordOrUndefined,
  findDuplicateHeaderKeys,
  findDuplicateKeys,
  formToCreateInput,
  formToUpdates,
  getEffectiveStdioCommand,
  getStdioLauncherUpdates,
  hasChanges,
  headerEntriesToRecordOrNull,
  headerEntriesToRecordOrUndefined,
  inferStdioLauncher,
} from "./server-form";
import type { ServerUpdateInput } from "@moor/types";

describe("server form utilities", () => {
  it("uses undefined for empty create fields and null for empty update fields", () => {
    expect(entriesToRecordOrUndefined([["", "ignored"]])).toBeUndefined();
    expect(entriesToRecordOrNull([["", "ignored"]])).toBeNull();
    expect(argsToArrayOrUndefined(" \n ")).toBeUndefined();
    expect(argsToArrayOrNull(" \n ")).toBeNull();
  });

  it("trims keys and argument lines while preserving values", () => {
    expect(
      entriesToRecordOrUndefined([
        [" TOKEN ", " secret "],
        ["EMPTY", ""],
      ]),
    ).toEqual({ TOKEN: " secret ", EMPTY: "" });
    expect(argsToArrayOrUndefined(" one \n\n two ")).toEqual(["one", "two"]);
  });

  it("parses pasted argument lines with Windows line endings", () => {
    expect(argsToArrayOrUndefined(" one\r\n\r\n two ")).toEqual(["one", "two"]);
  });

  it("normalizes HTTP header keys before creating payload records", () => {
    expect(
      headerEntriesToRecordOrUndefined([
        [" Authorization ", "Bearer token"],
        ["X-Trace", "abc"],
      ]),
    ).toEqual({ authorization: "Bearer token", "x-trace": "abc" });
    expect(headerEntriesToRecordOrNull([["", "ignored"]])).toBeNull();
  });

  it("reports every row that participates in a duplicate trimmed key", () => {
    expect(
      Array.from(
        findDuplicateKeys([
          [" TOKEN ", "a"],
          ["TOKEN", "b"],
          ["OTHER", "c"],
        ]),
      ),
    ).toEqual([0, 1]);
  });

  it("keeps environment keys case-sensitive but reports duplicate HTTP headers case-insensitively", () => {
    expect(
      Array.from(
        findDuplicateKeys([
          ["TOKEN", "a"],
          ["token", "b"],
        ]),
      ),
    ).toEqual([]);
    expect(
      Array.from(
        findDuplicateHeaderKeys([
          [" Authorization ", "Bearer a"],
          ["authorization", "Bearer b"],
          ["X-Other", "c"],
        ]),
      ),
    ).toEqual([0, 1]);
  });

  it("returns shared typed update payloads", () => {
    const stdioUpdates: ServerUpdateInput = formToUpdates(
      {
        name: " Local ",
        launcher: "command",
        command: " node ",
        url: "",
        args: " --stdio \n",
        env: [["TOKEN", "secret"]],
        headers: [],
        workingDir: " /tmp/moor ",
      },
      "stdio",
    );
    expect(stdioUpdates).toEqual({
      name: "Local",
      command: "node",
      args: ["--stdio"],
      env: { TOKEN: "secret" },
      workingDir: "/tmp/moor",
    });

    const httpUpdates: ServerUpdateInput = formToUpdates(
      {
        name: " Remote ",
        launcher: "command",
        command: "",
        url: " https://example.com/mcp ",
        args: "",
        env: [],
        headers: [[" Authorization ", "Bearer token"]],
        workingDir: "",
      },
      "http",
    );
    expect(httpUpdates).toEqual({
      name: "Remote",
      url: "https://example.com/mcp",
      headers: { authorization: "Bearer token" },
      env: null,
    });
  });

  it("builds a stdio create payload from an npx launcher preset", () => {
    expect(
      formToCreateInput({
        name: " Filesystem ",
        connectionType: "stdio",
        launcher: "npx",
        command: "ignored-command",
        args: " --yes \n @modelcontextprotocol/server-filesystem \n /data ",
        url: "",
        env: [["ROOT", "/data"]],
        headers: [],
        workingDir: " /data/project ",
        autoStart: true,
      }),
    ).toEqual({
      name: "Filesystem",
      connectionType: "stdio",
      command: "npx",
      args: ["--yes", "@modelcontextprotocol/server-filesystem", "/data"],
      env: { ROOT: "/data" },
      workingDir: "/data/project",
      autoStart: true,
    });
  });

  it("recognizes only exact built-in launcher commands", () => {
    expect([
      inferStdioLauncher("npx"),
      inferStdioLauncher("uvx"),
      inferStdioLauncher("/usr/local/bin/uvx"),
    ]).toEqual(["npx", "uvx", "command"]);
  });

  it("uses launcher presets as the effective stdio command", () => {
    expect([
      getEffectiveStdioCommand("command", " node "),
      getEffectiveStdioCommand("npx", "saved-command"),
      getEffectiveStdioCommand("uvx", "saved-command"),
    ]).toEqual(["node", "npx", "uvx"]);
  });

  it("adds non-interactive confirmation when selecting npx with empty arguments", () => {
    expect(getStdioLauncherUpdates("npx", " \n ")).toEqual({
      launcher: "npx",
      args: "--yes",
    });
  });

  it("preserves existing arguments when changing launchers", () => {
    expect(getStdioLauncherUpdates("npx", "--package\nexample")).toEqual({
      launcher: "npx",
      args: "--package\nexample",
    });
  });

  it("does not mark equivalent launcher projections dirty", () => {
    const baseline = {
      name: "Filesystem",
      launcher: "npx" as const,
      command: "npx",
      url: "",
      args: "--yes\n@modelcontextprotocol/server-filesystem\n/data",
      env: [],
      headers: [],
      workingDir: "",
    };

    expect(hasChanges({ ...baseline, launcher: "command" }, baseline)).toBe(false);
  });
});
