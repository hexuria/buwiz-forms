// Holds the real app open for an external agent: `just nsh-agent-check` runs this with --target macos (a real window, the host built
// with the `agent` feature) while tests/window_plane.rs drives the window through its control plane and the data plane.
// Headless (plain `nsh test`, no window to draw) it ends at once, like every shots test.
import { expect, open, test } from "nsh/test";
import { shoot } from "./shoot";

/** How long the window stays up for the agent; the recipe ends it earlier, when the agent is done. */
const HOLD_MS = 240_000;

test("the app stays up for an external agent", async () => {
  const app = await open("../App.tsx", { width: 1440, height: 900, platform: "macos" });
  expect(app.shows("id:setup-solo") || app.shows("id:nav-home")).toBe(true);
  if (!(await shoot(app, "agent-start"))) return;
  await new Promise<void>((resolve) => setTimeout(resolve, HOLD_MS));
}, HOLD_MS + 60_000);
