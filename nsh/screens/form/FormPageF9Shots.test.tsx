// F9 real-window pictures (headless they take none and pass). Run from the evidence folder so the PNGs land there:
//   cd ~/BuwizNsh/evidence/form-f9 && nsh-serial nsh test <worktree>/nsh/screens/form/FormPageF9Shots.test.tsx --target macos
import { open, test } from "nsh/test";
import { LOOKS, shoot } from "../../shots/shoot";

const ENTRY = "./FormPageAny.tsx";

for (const look of LOOKS.filter((l) => l.name !== "narrow-dark")) {
  test(`F9 2551Q Q3 2026 draft, ${look.name}`, async () => {
    const app = await open(ENTRY, { ...look, host: { params: { formCode: "2551Q", year: 2026, period: 3, seed: true } } });
    await app.waitFor(look.width > 1000 ? "id:fp-parts-toggle" : "id:fp-pill", { timeout: 15000 });
    await app.wait(800);
    await shoot(app, `f9-2551q-q3-${look.name}-top`);
    await app.press("id:fp-pill-next");
    await app.wait(1200);
    await shoot(app, `f9-2551q-q3-${look.name}-item13`);
  }, 120_000);
}

test("F9 compact form sections on, wide-light", async () => {
  const app = await open(ENTRY, { ...LOOKS[0], host: { params: { formCode: "2551Q", year: 2026, period: 3, seed: true, compact: true } } });
  await app.waitFor("id:fp-parts-toggle", { timeout: 15000 });
  await app.wait(800);
  await shoot(app, "f9-2551q-q3-compact-wide-light");
}, 120_000);

for (const code of ["0619E", "0619F", "1600WP", "1707A", "2550Q"]) {
  test(`F9 ${code} sample, wide-light`, async () => {
    const app = await open(ENTRY, { ...LOOKS[0], host: { params: { formCode: code, year: 0, period: 0, mode: "sample" } } });
    await app.waitFor("id:fp-parts-toggle", { timeout: 15000 });
    await app.wait(800);
    await shoot(app, `f9-${code}-top`);
  }, 120_000);
}
