import { describe, expect, it } from "vite-plus/test";
import { renderToStaticMarkup } from "react-dom/server";
import { StdioConfigFields } from "./StdioConfigFields";

describe("StdioConfigFields", () => {
  it("renders an accessible launcher radio group and custom command input", () => {
    const markup = renderToStaticMarkup(
      <StdioConfigFields
        idPrefix="test-server"
        launcher="command"
        command="node"
        args="server.js"
        workingDir="/data/project"
        onChange={() => undefined}
      />,
    );

    expect(markup).toContain("<legend");
    expect(markup).toContain(">Launcher</legend>");
    expect(markup.match(/type="radio"/g)).toHaveLength(3);
    expect(markup).toContain('id="test-server-command"');
    expect(markup).toContain('for="test-server-command"');
  });

  it("hides the custom command input for an npx preset", () => {
    const markup = renderToStaticMarkup(
      <StdioConfigFields
        idPrefix="test-server"
        launcher="npx"
        command="saved-command"
        args="--yes"
        workingDir=""
        onChange={() => undefined}
      />,
    );

    expect(markup).not.toContain('id="test-server-command"');
    expect(markup).toMatch(
      /<input[^>]*(?:checked=""[^>]*value="npx"|value="npx"[^>]*checked="")[^>]*>/,
    );
    expect(markup).toContain('id="test-server-working-dir"');
  });

  it("shows the current compatibility arguments for the uvx fetch example", () => {
    const markup = renderToStaticMarkup(
      <StdioConfigFields
        idPrefix="test-server"
        launcher="uvx"
        command=""
        args=""
        workingDir=""
        onChange={() => undefined}
      />,
    );

    expect(markup).toContain("--with\nmcp&lt;2\nmcp-server-fetch");
  });
});
