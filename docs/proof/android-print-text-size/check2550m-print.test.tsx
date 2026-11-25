// Scratch test (measurement only): opens Check2550m.tsx on the Android host, waits for the seed line,
// presses the real fp-print, then waits 8 minutes while adb drives the system print preview (Save as PDF).
import { open, test } from "nsh/test";

test("2550M seeds and fp-print opens the system print preview", async () => {
  const app = await open("./Check2550m.tsx");
  await app.waitFor("text:BIR Form 2550M", { timeout: 120_000 });
  await app.waitUntil(() => app.enabled("id:fp-print"), { timeout: 60_000 });
  await app.press("id:fp-print");
  await app.wait(480_000);
}, 600_000);
