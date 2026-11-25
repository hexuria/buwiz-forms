// F21: a return the dry-run worker sent says so on its page. FormSentShot.tsx queues the demo 1601C and the app's own dry-run worker sends
// it; the page then reads "Dry run <date>, not sent to BIR. Nothing here can be changed." and its chip "Dry run · read-only".
// Run: just nsh-shots f21 (~/BuwizNsh/evidence/f21).
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

// One look only: the worker starts once per process, on the first database an engine opens, so a second app in a headless run (one
// process) would wait for a send that never comes. (The narrow-dark picture in the evidence folder was taken by a two-look run.)
for (const look of LOOKS.filter((l) => l.name === "wide-light")) {
  test(`f21 ${look.name}: the dry-run send's page`, async () => {
    const app = await open("../FormSentShot.tsx", look);
    await app.waitUntil(() => app.texts().some((t) => /^Dry run .+, not sent to BIR\. Nothing here can be changed\.$/.test(t)), { timeout: 45_000 });
    expect(app.texts().some((t) => t.startsWith("Sent to BIR"))).toBe(false);
    // The status chip says the banner's word, not "Submitted".
    expect(app.texts()).toContain("Dry run · read-only");
    expect(app.texts()).not.toContain("Submitted · read-only");
    await shoot(app, `f21-${look.name}-dry-run`);
  }, 120_000);
}
