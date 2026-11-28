import { expect, open, test } from "nsh/test";

function report(name: string, times: number[]) {
  times.sort((a, b) => a - b);
  console.log(`SPEED ${name}: n=${times.length}; p50=${times[Math.floor(times.length * .5)]}ms; p95=${times[Math.floor(times.length * .95)]}ms; max=${times[times.length - 1]}ms`);
}

for (const formCode of ["1701", "1702RT"]) {
  test(`speed annual ${formCode}: scrolling and 100 accepted keystrokes`, async () => {
    const start = Date.now();
    const app = await open("./FormPageAny.tsx", {
      width: 1440, height: 900, platform: "macos", clock: {},
      host: { params: { formCode, year: 2025, period: 0, openParts: "all" } },
    });
    await app.waitFor(`text:BIR Form ${formCode}`);
    await app.waitUntil(() => app.texts().some(t => /^session fields [1-9]/.test(t)), { timeout: 10000 });
    console.log(`SPEED ${formCode} initial open=${Date.now() - start}ms; ${app.texts().find(t => /^session fields/.test(t))}`);
    const scroll: number[] = [];
    for (let i = 0; i < 30; i++) {
      const now = Date.now();
      await app.scroll("id:form-scroll", i < 15 ? 800 : -800);
      scroll.push(Date.now() - now);
    }
    report(`${formCode} headless scroll API (gesture, not frame)`, scroll);
    const input = formCode === "1701" ? "id:fp-frm1701:txtPg1I14ForeignTaxNumber" : "id:fp-frm1702RT:txtPg2Pt4I27Sales";
    if (formCode === "1701") {
      const yes = "id:fp-frm1701:rdoPg1I13ForeignTaxCreditsYes";
      for (let i = 0; i < 30 && !app.shows(yes); i++) await app.scroll("id:form-scroll", -800);
      await app.press(yes);
    }
    for (let i = 0; i < 30 && !app.shows(input); i++) await app.scroll("id:form-scroll", 800);
    expect(app.count(input)).toBe(1);
    const typing: number[] = [];
    for (let i = 0; i < 100; i++) {
      await app.clear(input);
      const now = Date.now();
      await app.type(input, "1");
      typing.push(Date.now() - now);
      expect(app.value(input)).toBe("1");
    }
    report(`${formCode} headless accepted keystroke API`, typing);
  }, 60000);
}
