// F3 by hand: 1702MX Schedules 7.1/8.1 NOLCO rows take entries once Itemized is chosen.
import { expect, open, test } from "nsh/test";

test("phone: schedule 7.1 row 1 amounts are fillable", async () => {
  const app = await open("./FormPageAny.tsx", {
    host: { params: { formCode: "1702MX", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1702MX", { timeout: 20_000 });
  await app.wait(2000);
  expect(app.texts().some((t) => t.includes("Can't be filled on this page yet."))).toBe(false);
}, 60_000);
