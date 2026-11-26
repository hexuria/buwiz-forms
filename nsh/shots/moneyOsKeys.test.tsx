import { open, test } from "nsh/test";

// OS-key probe: the test only navigates and reads; every amount key is typed by
// xdotool against the xvfb display, driven by the WANT lines this test logs.
const VALUES = ["99999999.99", "12345678.90", "123456789.12", "1000000.005", "1234567.89"];

async function osType(app: any, box: string, v: string, mode: string, label: string) {
  await app.clear(box);
  await app.wait(300);
  console.log(`WANT:${mode}:${v}`);
  const perKey = mode === "burst" ? 0.05 : 0.3;
  await app.wait(Math.ceil(v.length * perKey * 1000) + 5000);
  console.log(`RESULT ${label} ${mode} ${v} inflight-box '${app.value(box)}'`);
}

test("sched1 os keys", async () => {
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
  for (const v of VALUES) {
    for (const mode of ["type120", "burst"] as const) {
      await app.press("text:/PT010 ·/");
      await app.waitFor(amt, { timeout: 5000 });
      await osType(app, amt, v, mode, "sched1");
      await app.press("text:Done");
      await app.wait(1200);
      await app.press("text:/PT010 ·/");
      await app.waitFor(amt, { timeout: 5000 });
      console.log(`RESULT sched1 ${mode} ${v} saved-box '${app.value(amt)}'`);
      await app.press("text:Done");
      await app.wait(400);
    }
  }
  // Mid-amount editing: fresh 10 digits, then Backspace and a digit in the middle.
  await app.press("text:/PT010 ·/");
  await app.waitFor(amt, { timeout: 5000 });
  await osType(app, amt, "1234567890", "type120", "sched1-midsetup");
  console.log("WANT:keys:Left Left Left Left BackSpace");
  await app.wait(5000);
  console.log(`RESULT sched1 mid-backspace inflight-box '${app.value(amt)}'`);
  console.log("WANT:keys:9");
  await app.wait(4000);
  console.log(`RESULT sched1 mid-insert inflight-box '${app.value(amt)}'`);
  await app.press("text:Done");
  await app.wait(1200);
  await app.press("text:/PT010 ·/");
  await app.waitFor(amt, { timeout: 5000 });
  console.log(`RESULT sched1 mid-edit saved-box '${app.value(amt)}'`);
  await app.press("text:Done");
}, 300_000);

test("item15 amended os keys", async () => {
  const app = await open("../screens/form/FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: { params: { formCode: "2551Q", year: 2025, period: 3, openParts: "all", edits: { "frm2551Qv2018:amendedRtn": "Yes" } } },
  });
  await app.waitFor("text:BIR Form 2551Q");
  const box = "id:fp-frm2551Qv2018:txt15";
  for (let i = 0; i < 20 && !app.shows(box); i++) await app.scroll("id:form-scroll", 700).catch(() => {});
  for (const v of VALUES) {
    for (const mode of ["type120", "burst"] as const) {
      await osType(app, box, v, mode, "item15");
      await app.submit(box).catch(() => {});
      await app.wait(1500);
      console.log(`RESULT item15 ${mode} ${v} saved-box '${app.value(box)}' taxdue14 '${app.value("id:fp-frm2551Qv2018:txt14")}'`);
    }
  }
}, 300_000);

test("1701 and 2550M money boxes os keys", async () => {
  for (const form of ["1701", "2550M"]) {
    const app = await open("../screens/form/FormPageAny.tsx", {
      width: 1440, height: 900, platform: "macos", clock: {},
      host: { params: { formCode: form, year: 2025, period: 0, openParts: "all" } },
    });
    await app.waitFor(`text:BIR Form ${form.replace("M", "M")}`);
    // First plain money box that is not computed: probe candidates by scanning inputs.
    await app.wait(1500);
    console.log(`RESULT ${form} money scan:`, app.texts().filter((t) => /^\d+$/.test(t)).slice(0, 10).join("|"));
    app.close?.();
  }
}, 120_000);
