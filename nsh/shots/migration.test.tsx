// Real-window screenshots of the migration pages (screens/auth/migration.tsx, through MigrationDemo.tsx):
// Moving your tax profiles, Could not be moved yet (before and after the copy exists), Moved not opened, and the
// offer, light and dark, wide and narrow. `just nsh-shots migration` saves ~/BuwizNsh/evidence/migration/*.png.
// Headless (plain nsh test) it takes no pictures and passes.
import { open, test } from "nsh/test";
import { AUTH_LOOKS, shoot } from "./shoot";

const DEMO = "../MigrationDemo.tsx";

for (const look of AUTH_LOOKS) {
  const size = { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme };
  const at = (page: string) => open(DEMO, { ...size, host: { params: { page } } });

  test(`migration pages, ${look.name}`, async () => {
    let app = await at("moving");
    await app.waitFor("id:migration-moving");
    await app.wait(1200);
    await shoot(app, `migration-${look.name}-moving`);


    app = await at("transient");
    await app.waitFor("id:migration-transient");
    await app.wait(400);
    await shoot(app, `migration-${look.name}-transient`);


    app = await at("transient-copy");
    await app.waitFor("id:migration-transient");
    await app.wait(400);
    await shoot(app, `migration-${look.name}-transient-copy`);


    app = await at("moved");
    await app.waitFor("id:migration-moved-not-opened");
    await app.wait(400);
    await shoot(app, `migration-${look.name}-moved`);


    app = await at("offered");
    await app.waitFor("id:migration-offer");
    await app.wait(400);
    await shoot(app, `migration-${look.name}-offered`);

  }, 120_000);
}
