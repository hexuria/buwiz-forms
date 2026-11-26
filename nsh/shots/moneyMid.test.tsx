import { open, test } from "nsh/test";

test("sched1 mid-edit os keys", async () => {
  const app = await open("../screens/form/FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: { params: { formCode: "2551Q", year: 2025, period: 3, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-check");
  await app.waitFor("id:fp-pill-count");
  await app.press("id:fp-pill-open");
  await app.waitFor("id:fp-section-4", { timeout: 15_000 });
  await app.press("id:fp-section-4");
  await app.wait(900);
  await app.waitFor("text:/PT010 ·/", { timeout: 8000 });
  const amt = "id:fp-txtATCAmt1";
  await app.press("text:/PT010 ·/");
  await app.waitFor(amt, { timeout: 5000 });
  await app.clear(amt);
  console.log("WANT:type120:1234567890");
  await app.wait(9000);
  console.log(`RESULT midsetup inflight '${app.value(amt)}'`);
  console.log("WANT:keys:Left Left Left Left BackSpace");
  await app.wait(5000);
  console.log(`RESULT mid-backspace inflight '${app.value(amt)}'`);
  console.log("WANT:keys:9");
  await app.wait(4000);
  console.log(`RESULT mid-insert inflight '${app.value(amt)}'`);
  await app.press("text:Done");
  await app.wait(1200);
  await app.press("text:/PT010 ·/");
  await app.waitFor(amt, { timeout: 5000 });
  console.log(`RESULT mid-edit saved '${app.value(amt)}'`);
  // also: the two values that never completed in the first run
  for (const v of ["1000000.005", "1234567.89"]) {
    await app.press("text:Done");
    await app.wait(400);
    await app.press("text:/PT010 ·/");
    await app.waitFor(amt, { timeout: 5000 });
    await app.clear(amt);
    console.log(`WANT:type120:${v}`);
    await app.wait(9000);
    console.log(`RESULT sched1 ${v} inflight '${app.value(amt)}'`);
    await app.press("text:Done");
    await app.wait(1200);
    await app.press("text:/PT010 ·/");
    await app.waitFor(amt, { timeout: 5000 });
    console.log(`RESULT sched1 ${v} saved '${app.value(amt)}'`);
  }
  await app.press("text:Done");
}, 180_000);
