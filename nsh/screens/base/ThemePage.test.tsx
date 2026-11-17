// The base screen at phone size (the default viewport, 390 x 844) and at a desktop size, and the Rust bridge through it.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../BasePreview.tsx";

test("phone: narrow layout, and the Rust bridge answers", async () => {
  const app = await open(ENTRY); // 390 x 844, platform "ios", light
  expect(app.texts()).toContain("ios | light | 390 x 844 pt | narrow");
  expect(app.texts()).toContain("Native module says: pong");
  expect(app.texts()).toContain("Pings: 1");
  await app.press("role:button[Ping the Rust bridge]");
  expect(app.texts()).toContain("Pings: 2");
});

test("desktop: wide layout as macOS, in dark", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", colorScheme: "dark" });
  expect(app.texts()).toContain("macos | dark | 1440 x 900 pt | wide");
  expect(app.texts()).toContain("Native module says: pong");
  await app.setViewport(500, 700); // the window narrows: the layout follows
  expect(app.texts()).toContain("macos | dark | 500 x 700 pt | narrow");
});
