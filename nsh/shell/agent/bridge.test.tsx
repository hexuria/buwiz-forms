// The agent's commands on the real frame, headless: each command the host would queue is applied to the app as the old app applied
// it (go, open, choose, review, refresh). The fixture feeds one command per press, through the same bridge the app root starts.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../AgentDemo.tsx";
const TIN = "00012345300000";
const SHEETS = "frm2551Qv2018:txtSheets";
const YEAR = new Date().getFullYear();

const COMMANDS = [
  JSON.stringify({ op: "refresh", tin_root: "000123453" }),
  JSON.stringify({ op: "go", view: "filings" }),
  JSON.stringify({ op: "open", tin: TIN, code: "2551Q", year: YEAR, period: 1 }),
  JSON.stringify({ op: "choose", edits: { [SHEETS]: "3" } }),
  JSON.stringify({ op: "review", message: "Prepared by the assistant. Review every box, then click Queue yourself." }),
  JSON.stringify({ op: "go", view: "library" }),
];

async function next(app: Awaited<ReturnType<typeof open>>) {
  await app.press("id:agent-next");
}

test("desktop: go, open, choose and review reach the frame in order", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", host: { params: { commands: COMMANDS } } });
  await app.waitFor("id:nav-home");

  await next(app); // refresh: the taxpayer is selected, nothing else moves
  await next(app); // go filings
  await app.waitFor("text:Filings");
  expect(app.count("id:fp-header")).toBe(0);

  await next(app); // open the 2551Q
  await app.waitFor("id:fp-header", { timeout: 20_000 });
  await app.waitFor("text:BIR Form 2551Q");
  expect(app.count("id:fp-agent-review")).toBe(0);

  await next(app); // choose: the box shows what the agent wrote
  await app.waitUntil(() => app.value(`id:fp-${SHEETS}`) === "3", { timeout: 20_000 });

  await next(app); // review: the page says so and a toast says it too
  await app.waitFor("id:fp-agent-review");
  await app.waitFor("text:/^Prepared by the assistant\\. Review every box/");

  await next(app); // leaving the form takes the review with it
  await app.waitFor("text:Form library");
  await app.press("id:nav-home");
  await app.waitFor("id:nav-home");
  expect(app.count("id:fp-agent-review")).toBe(0);
});

test("phone: the same commands apply under 600 points", async () => {
  const app = await open(ENTRY, { host: { params: { commands: COMMANDS } } });
  await app.waitFor("id:agent-next");
  for (let i = 0; i < 3; i++) await next(app); // refresh, go filings, open
  await app.waitFor("id:fp-header", { timeout: 20_000 });
  await next(app); // choose
  await app.waitUntil(() => app.value(`id:fp-${SHEETS}`) === "3", { timeout: 20_000 });
  await next(app); // review
  await app.waitFor("id:fp-agent-review");
});

test("desktop: a taxpayer the host wrote behind the window's back, then a refresh and a view change, on the first-run database", async () => {
  const commands = [
    JSON.stringify({ op: "refresh", tin_root: "000123453" }),
    JSON.stringify({ op: "go", view: "filings" }),
  ];
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", host: { params: { commands, fresh: true } } });
  await app.waitFor("id:setup-solo");
  await app.press("id:setup-solo");
  await app.waitFor("id:new-tp-name");
  await next(app); // the taxpayer is written, then the refresh is applied
  await app.waitUntil(() => app.count("id:new-tp-name") === 0 || app.shows("id:nav-home"), { timeout: 20_000 });
  await next(app); // go filings
  await app.waitFor("id:nav-filings", { timeout: 20_000 });
});

test("F8-3 desktop: commands that arrive while the sign-in screen is up are dropped, and the frame takes them again once signed in", async () => {
  const commands = [JSON.stringify({ op: "go", view: "filings" }), JSON.stringify({ op: "go", view: "library" })];
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", host: { params: { commands, pin: true } } });
  await app.waitFor("id:auth-card");
  await next(app); // go filings, while locked
  await app.wait(300);
  await app.type("id:auth-tin", "000123453");
  await app.type("id:auth-tp-pin", "1234");
  await app.waitFor("id:frame");
  await app.wait(300);
  expect(app.shows("id:filings-search")).toBe(false);
  await next(app); // go library, signed in
  await app.waitFor("text:Form library");
});
