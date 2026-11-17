// F8-12: the frame boots with the dev start route only where the door reports a test environment. The route's access steps take
// PINs and the recovery key (signin=<root>:<pin>, override=, recover=, setuppin=), so a shipped app must not read them from its
// environment. One boot per file (boot is idempotent): this file boots as a shipped app would, and tests of the route in a test
// environment are every frame test that passes `start`.
import { expect, test } from "nsh/test";
import { app, boot } from "./app";

test("booted as a shipped app, the start route is ignored: its steps, its theme and its keys", async () => {
  const steps: string[] = [];
  app.setAccessSteps((key, value) => steps.push(`${key}=${value}`));
  await boot("theme=dark;view=settings;signin=000123453:1234;recover=ABCD-EFGH", { testEnvironment: false });
  expect(app.booted()).toBe(true);
  expect(steps).toEqual([]);
  expect(app.startParams()).toEqual({});
  expect(app.themePref()).toBe("system");
  expect(app.route().view).toBe("home");
  app.setAccessSteps(null);
});
