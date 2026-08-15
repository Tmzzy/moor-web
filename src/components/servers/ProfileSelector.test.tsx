import { describe, expect, it } from "vite-plus/test";
import { renderToStaticMarkup } from "react-dom/server";

import { ProfileSelector } from "./ProfileSelector";

const profiles = [
  {
    id: "personal",
    name: "Personal",
    serverCount: 1,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  },
  {
    id: "work",
    name: "Work",
    serverCount: 1,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  },
];

describe("ProfileSelector", () => {
  it("renders all profile choices and marks the selected scope", () => {
    const markup = renderToStaticMarkup(
      <ProfileSelector profiles={profiles} selectedIds={["personal"]} onChange={() => undefined} />,
    );

    expect(markup).toContain("All current profiles");
    expect(markup).toContain("Personal");
    expect(markup).toContain("Work");
    expect(markup).toContain("1 / 2");
    expect(markup).toContain('aria-label="Use server in Personal"');
    expect(markup).not.toContain("Default");
  });
});
