// F5-6: the controls of a multi-control row sit on one line. A searchable pick (a long choice) used to draw its closed
// sheet as a second child of the box's column, whose gap pushed the column 4 points taller: bottom-aligned, the pick and
// its label sat 4 points above the box beside it (2551Q Item 2, "Year Ended MM" and "Year").
import { expect, open, test } from "nsh/test";

const frame = (texts: string[], name: string) => {
  const t = texts.find((x) => x.startsWith(name + " "))!;
  const m = /y=(-?[\d.]+) h=(-?[\d.]+)/.exec(t)!;
  return { y: Number(m[1]), h: Number(m[2]) };
};

for (const look of [{ name: "desktop", o: { width: 1440, height: 900, platform: "macos" as const } }, { name: "phone", o: undefined }]) {
  test(`${look.name}: a long choice and a locked box of one row share their top and their height`, async () => {
    const app = await open("./RowAlign.tsx", look.o);
    await app.waitFor("text:/^colB /");
    await app.waitFor("text:/^ctlA /");
    const texts = app.texts();
    const [colA, colB, ctlA, ctlB] = ["colA", "colB", "ctlA", "ctlB"].map((n) => frame(texts, n));
    expect(colA.y).toBe(colB.y);
    expect(colA.h).toBe(colB.h);
    expect(ctlA.y).toBe(ctlB.y);
    expect(ctlA.h).toBe(ctlB.h);
  });
}
