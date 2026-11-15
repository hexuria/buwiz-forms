// Real-window screenshots of the Form library: light and dark, wide and narrow, with the demo taxpayer (ShellDemo.tsx).
// `just nsh-shots library` saves ~/BuwizNsh/evidence/library/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const ENTRY = "../ShellDemo.tsx";

for (const look of LOOKS) {
  test(`library, ${look.name}`, async () => {
    const app = await open(ENTRY, {
      width: look.width,
      height: look.height,
      platform: look.platform,
      colorScheme: look.colorScheme,
      host: { params: { start: look.colorScheme === "dark" ? "view=library;theme=dark" : "view=library" } },
    });
    await app.waitFor("id:library-grid");
    await app.wait(500);
    await shoot(app, `library-${look.name}-filed`);
    await app.press("id:library-scope >> text:/^All forms/");
    await app.waitFor("text:43 of 43");
    await app.wait(300);
    await shoot(app, `library-${look.name}-all`);
    await app.scroll("id:main-scroll", 600);
    await app.wait(300);
    await shoot(app, `library-${look.name}-all-scrolled`);
    await app.type("id:library-search", "vat");
    await app.wait(300);
    await shoot(app, `library-${look.name}-vat`);
  }, 120_000);
}
