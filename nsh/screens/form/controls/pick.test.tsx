// SearchPick, the long choice (RDO, ZIP, ATC): at phone size a sheet whose search field stays pinned over its own scrolling list,
// at a desktop a combobox whose box is the text (keys Up/Down/Enter/Escape/Tab; text that is no choice is not taken on leaving).
// The page is PickProbe.tsx: an RDO list of 138 and a ZIP list of 400, each printing what it reported, and the sheet's frames.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const ENTRY = "./PickProbe.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };

async function probe(params: Record<string, unknown> = {}, options: Record<string, unknown> = {}): Promise<App> {
  const app = await open(ENTRY, { ...options, host: { params } });
  await app.waitFor("text:Pick probe");
  return app;
}

/** The fixture's line for one pick: "rdo: 050 | picks: 1 | opens: 1". */
const line = (app: App, name: "rdo" | "zip") => app.texts().find((t) => t.startsWith(`${name}:`)) ?? "";

/** Scrolls a list by `step` until `selector` shows or the list ends (app.scroll throws past an end: "at X of Y"). */
async function scrollTo(app: App, list: string, step: number, selector: string) {
  for (let i = 0; i < 60 && !app.shows(selector); i++) {
    try {
      await app.scroll(list, step);
    } catch {
      break;
    }
  }
  await app.waitFor(selector);
}

/** How many times the sheet reported its frames ("frames reports: 4"). */
const reports = (app: App) => Number((app.texts().find((t) => t.startsWith("frames reports:")) ?? "frames reports: 0").slice(16));

/**
 * "search=173-217 list=229-828 panel=173-828 window=844 header=96.4~157.4 body=157.4~844 sheet=84.4 room=686.6 max=759.6" as
 * numbers: the pick's own parts in whole points, the sheet's (its header and body views, its panel's top, what sheetBodyRoom() and
 * sheetPanelMaxHeight() said) to a tenth.
 */
function frames(app: App, which: "first" | "last") {
  const text = app.texts().find((t) => t.startsWith(`frames ${which}:`)) ?? "";
  const part = (name: string) => {
    const m = new RegExp(`${name}=(-?\\d+)-(-?\\d+)`).exec(text);
    return m ? { top: Number(m[1]), bottom: Number(m[2]) } : null;
  };
  const fine = (name: string) => {
    const m = new RegExp(`${name}=(-?[\\d.]+)~(-?[\\d.]+)`).exec(text);
    return m ? { top: Number(m[1]), bottom: Number(m[2]) } : null;
  };
  const num = (name: string) => {
    const m = new RegExp(`${name}=(-?[\\d.]+)`).exec(text);
    return m ? Number(m[1]) : null;
  };
  const w = /window=(\d+)/.exec(text);
  return {
    search: part("search"),
    list: part("list"),
    panel: part("panel"),
    window: w ? Number(w[1]) : 0,
    header: fine("header"),
    body: fine("body"),
    sheetTop: num("sheet"),
    room: num("room"),
    max: num("max"),
  };
}

// ---------------------------------------------------------------- phone

test("phone: the trigger opens a sheet with the search field; words in any order filter; a row picks and closes", async () => {
  const app = await probe();
  await app.press("id:pp-rdo-trigger");
  await app.waitFor("id:pp-rdo-search");
  expect(line(app, "rdo")).toBe("rdo:  | picks: 0 | opens: 1");
  await app.waitFor("id:pp-rdo-opt-001");
  await app.type("id:pp-rdo-search", "makati south");
  await app.waitFor("id:pp-rdo-opt-050");
  expect(app.count("id:pp-rdo-opt-047")).toBe(0);
  expect(app.count("id:pp-rdo-opt-001")).toBe(0);
  await app.press("id:pp-rdo-opt-050");
  await app.waitFor("id:pp-rdo-search", { state: "gone" });
  expect(line(app, "rdo")).toBe("rdo: 050 | picks: 1 | opens: 1");
  // The trigger says the choice as code and name.
  expect(app.texts()).toContain("050 — South Makati");
});

test("phone: a ZIP is found by its number, and a search that finds nothing says so", async () => {
  const app = await probe();
  await app.press("id:pp-zip-trigger");
  await app.waitFor("id:pp-zip-search");
  await app.type("id:pp-zip-search", "zzz");
  await app.waitFor("text:Nothing matches. Try a code or other words.");
  expect(app.count("id:pp-zip-opt-1000")).toBe(0);
  await app.clear("id:pp-zip-search");
  await app.type("id:pp-zip-search", "1226");
  await app.waitFor("id:pp-zip-opt-1226");
  await app.press("id:pp-zip-opt-1226");
  await app.waitFor("id:pp-zip-search", { state: "gone" });
  expect(line(app, "zip")).toBe("zip: 1226 | picks: 1 | opens: 1");
});

test("phone: a filer who does not know the ZIP finds it by the city's name", async () => {
  const app = await probe();
  await app.press("id:pp-zip-trigger");
  await app.waitFor("id:pp-zip-search");
  await app.type("id:pp-zip-search", "makati");
  await app.waitFor("id:pp-zip-opt-1209");
  expect(app.count("id:pp-zip-opt-1226")).toBe(1);
  expect(app.count("id:pp-zip-opt-1232")).toBe(1);
  expect(app.count("id:pp-zip-opt-1000")).toBe(0);
  // Words in any order narrow it to one city part.
  await app.clear("id:pp-zip-search");
  await app.type("id:pp-zip-search", "makati poblacion");
  await app.waitFor("id:pp-zip-opt-1232");
  expect(app.count("id:pp-zip-opt-1209")).toBe(0);
  await app.press("id:pp-zip-opt-1232");
  await app.waitFor("id:pp-zip-search", { state: "gone" });
  expect(line(app, "zip")).toBe("zip: 1232 | picks: 1 | opens: 1");
});

test("phone: the list opens on the current choice, and the clear row clears it", async () => {
  const app = await probe({ rdo: "120" });
  await app.press("id:pp-rdo-trigger");
  await app.waitFor("id:pp-rdo-opt-120");
  // 119 rows above it: the list starts there, not at the top.
  expect(app.count("id:pp-rdo-opt-001")).toBe(0);
  await app.back();
  await app.waitFor("id:pp-rdo-search", { state: "gone" });
  await app.press("id:pp-rdo-trigger");
  await app.waitFor("id:pp-rdo-search");
  // The clear row heads the list: scroll back to the top for it.
  await scrollTo(app, "id:pp-rdo-list", -1500, "id:pp-rdo-opt-");
  await app.press("id:pp-rdo-opt-");
  await app.waitFor("id:pp-rdo-search", { state: "gone" });
  expect(line(app, "rdo")).toBe("rdo:  | picks: 1 | opens: 2");
});

test("phone: the search field stays where it is, inside the sheet, while the list scrolls to its end", async () => {
  const app = await probe();
  await app.press("id:pp-rdo-trigger");
  await app.waitFor("id:pp-rdo-search");
  // Measured as the sheet is laid out (the fixture prints the sheet's frames; the page behind the sheet is still read by texts()).
  await app.waitUntil(() => frames(app, "first").search !== null);
  const opened = reports(app);
  await scrollTo(app, "id:pp-rdo-list", 600, "id:pp-rdo-opt-138");
  // Scrolled to its very end (the last step may stop short of it).
  try {
    await app.scroll("id:pp-rdo-list", 600);
  } catch {
    // at the end already
  }
  // The scrolls measured the frames again: "last" is the sheet with its list at the end.
  await app.waitUntil(() => reports(app) > opened);
  const before = frames(app, "first");
  const after = frames(app, "last");
  for (const f of [before, after]) {
    expect(f.search).not.toBeNull();
    expect(f.list).not.toBeNull();
    expect(f.panel).not.toBeNull();
  }
  // Pinned: the field did not move with the list (the bug: the sheet's body scrolled, and the field went up and out with the rows).
  expect(after.search!.top).toBe(before.search!.top);
  expect(after.list!.top).toBe(before.list!.top);
  // Wholly inside the sheet: under the panel's top, above the list, on the screen; the list ends inside the panel, which ends on it.
  expect(after.search!.top).toBeGreaterThanOrEqual(after.panel!.top);
  expect(after.search!.top).toBeGreaterThanOrEqual(0);
  expect(after.search!.bottom).toBeLessThanOrEqual(after.list!.top);
  expect(after.list!.bottom).toBeLessThanOrEqual(after.panel!.bottom + 1);
  expect(after.panel!.bottom).toBeLessThanOrEqual(after.window);
  // The last row is reachable: a press on it picks it.
  await app.press("id:pp-rdo-opt-138");
  await app.waitFor("id:pp-rdo-search", { state: "gone" });
  expect(line(app, "rdo")).toBe("rdo: 138 | picks: 1 | opens: 1");
});

test("phone: the list's room is the sheet's real body: sheetBodyRoom() is the body Sheet draws, and the list ends inside it", async () => {
  const app = await probe();
  await app.press("id:pp-rdo-trigger");
  await app.waitFor("id:pp-rdo-search");
  await app.waitUntil(() => frames(app, "last").header !== null);
  const f = frames(app, "last");
  expect(f.list).not.toBeNull();
  expect(f.body).not.toBeNull();
  expect(f.sheetTop).not.toBeNull();
  expect(f.room).not.toBeNull();
  expect(f.max).not.toBeNull();
  const header = f.header!;
  const body = f.body!;
  // The body starts where Sheet's header ends.
  expect(Math.abs(body.top - header.bottom)).toBeLessThanOrEqual(1);
  // The list ends inside the body (a header taller than sheetBodyRoom() counts makes the list taller than the room it has).
  expect(f.list!.bottom).toBeLessThanOrEqual(body.bottom);
  // sheetBodyRoom() is the body's measured height: its grabber and header numbers are Sheet's real layout. A taller header
  // clamps the panel and leaves the body less than the room the list was sized to.
  expect(Math.abs(body.bottom - body.top - f.room!)).toBeLessThanOrEqual(1);
  // And the panel is as tall as it may be (sheetPanelMaxHeight), from its measured top to the window's bottom (no keyboard, no
  // inset): a shorter header would leave the panel short of it, the room unused.
  expect(Math.abs(f.window - f.sheetTop! - f.max!)).toBeLessThanOrEqual(1);
});

// ---------------------------------------------------------------- desktop

test("desktop: the box is the search; typing opens the list under it; a row picks and the box shows code and name", async () => {
  const app = await probe({}, DESKTOP);
  await app.type("id:pp-rdo-search", "makati south");
  await app.waitFor("id:pp-rdo-opt-050");
  expect(app.count("id:pp-rdo-opt-047")).toBe(0);
  expect(line(app, "rdo")).toBe("rdo:  | picks: 0 | opens: 1");
  await app.press("id:pp-rdo-opt-050");
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  expect(line(app, "rdo")).toBe("rdo: 050 | picks: 1 | opens: 1");
  expect(app.value("id:pp-rdo-search")).toBe("050 — South Makati");
});

test("desktop: the chevron opens the whole list and closes it again; nothing matching says so", async () => {
  const app = await probe({}, DESKTOP);
  await app.press("id:pp-rdo-trigger");
  await app.waitFor("id:pp-rdo-list");
  expect(app.count("id:pp-rdo-opt-001")).toBe(1);
  expect(line(app, "rdo")).toBe("rdo:  | picks: 0 | opens: 1");
  await app.press("id:pp-rdo-trigger");
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  await app.type("id:pp-rdo-search", "zzz");
  await app.waitFor("text:Nothing matches. Try a code or other words.");
  // The sentence has an id of its own (the list is not drawn when nothing matches), for recipes that wait on it.
  await app.waitFor("id:pp-rdo-nothing");
  expect(app.shows("id:pp-rdo-list")).toBe(false);
});

test("desktop: the focus leaves the RDO box and the ZIP chevron is pressed at once: the press reaches ZIP, not the RDO list", async () => {
  // The cause of a batch's flake (KNOWN-FLAKES.md, setup.test.tsx): a leaving that kept the list open for a moment (#425's 250 ms
  // grace) left it over the next box, so a press there right after landed on the RDO list. The blur now settles at once.
  const app = await probe({ rdo: "050" }, DESKTOP);
  await app.clear("id:pp-rdo-search");
  await app.type("id:pp-rdo-search", "makati");
  await app.waitFor("id:pp-rdo-opt-047");
  await app.keyboard.press("tab");
  await app.press("id:pp-zip-trigger");
  await app.waitFor("id:pp-zip-list");
  expect(app.shows("id:pp-rdo-list")).toBe(false);
  expect(line(app, "zip")).toBe("zip:  | picks: 0 | opens: 1");
  expect(line(app, "rdo")).toBe("rdo: 050 | picks: 0 | opens: 1");
});

test("desktop: Up and Down move the highlight, Enter picks the lit row", async () => {
  const app = await probe({}, DESKTOP);
  await app.type("id:pp-rdo-search", "makati");
  await app.waitFor("id:pp-rdo-opt-050");
  const lit = (code: string) => Boolean(app.viewStyle(`id:pp-rdo-opt-${code}`)?.backgroundColor);
  // The first match is lit as the list opens.
  expect(lit("047")).toBe(true);
  expect(lit("050")).toBe(false);
  await app.keyboard.press("down");
  expect(lit("047")).toBe(false);
  expect(lit("050")).toBe(true);
  // The end holds; Up goes back.
  await app.keyboard.press("down");
  expect(lit("050")).toBe(true);
  await app.keyboard.press("up");
  expect(lit("047")).toBe(true);
  await app.keyboard.press("down");
  await app.keyboard.press("enter");
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  expect(line(app, "rdo")).toBe("rdo: 050 | picks: 1 | opens: 1");
  expect(app.value("id:pp-rdo-search")).toBe("050 — South Makati");
  // Down on the closed box opens the list again.
  await app.keyboard.press("down");
  await app.waitFor("id:pp-rdo-list");
});

test("desktop: Escape closes the list and puts the choice back in the box", async () => {
  const app = await probe({ rdo: "050" }, DESKTOP);
  expect(app.value("id:pp-rdo-search")).toBe("050 — South Makati");
  await app.clear("id:pp-rdo-search");
  await app.type("id:pp-rdo-search", "east");
  await app.waitFor("id:pp-rdo-opt-047");
  await app.keyboard.press("escape");
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  expect(app.value("id:pp-rdo-search")).toBe("050 — South Makati");
  expect(line(app, "rdo")).toBe("rdo: 050 | picks: 0 | opens: 1");
  expect(app.shows("id:pp-rdo-note")).toBe(false);
});

test("desktop: Tab takes a code typed whole, and goes back to the last pick with a sentence for text that is no choice", async () => {
  const app = await probe({ rdo: "050" }, DESKTOP);
  await app.clear("id:pp-rdo-search");
  await app.type("id:pp-rdo-search", "xyz");
  await app.keyboard.press("tab");
  await app.waitFor("text:“xyz” is not in the list. The box kept 050 — South Makati.");
  expect(app.value("id:pp-rdo-search")).toBe("050 — South Makati");
  expect(line(app, "rdo")).toBe("rdo: 050 | picks: 0 | opens: 1");
  expect(app.shows("id:pp-rdo-list")).toBe(false);
  // A code typed whole is taken on leaving, and the sentence goes.
  await app.clear("id:pp-rdo-search");
  await app.type("id:pp-rdo-search", "047");
  await app.waitFor("text:“xyz” is not in the list. The box kept 050 — South Makati.", { state: "gone" });
  await app.keyboard.press("tab");
  await app.waitUntil(() => line(app, "rdo") === "rdo: 047 | picks: 1 | opens: 2");
  expect(app.value("id:pp-rdo-search")).toBe("047 — East Makati");
});

test("desktop: an emptied box clears the choice on leaving; a required one goes back and says so", async () => {
  const free = await probe({ rdo: "050" }, DESKTOP);
  await free.clear("id:pp-rdo-search");
  await free.keyboard.press("tab");
  await free.waitUntil(() => line(free, "rdo") === "rdo:  | picks: 1 | opens: 1");
  expect(free.value("id:pp-rdo-search")).toBe("");

  const required = await probe({ rdo: "050", required: true }, DESKTOP);
  await required.clear("id:pp-rdo-search");
  await required.keyboard.press("tab");
  await required.waitFor("text:This choice can't be left empty. The box kept 050 — South Makati.");
  expect(required.value("id:pp-rdo-search")).toBe("050 — South Makati");
  expect(line(required, "rdo")).toBe("rdo: 050 | picks: 0 | opens: 1");
});

test("desktop: the window going inactive (Alt+Tab) closes the list and settles the typed text, as a native popup", async () => {
  const app = await probe({ rdo: "050" }, DESKTOP);
  await app.clear("id:pp-rdo-search");
  await app.type("id:pp-rdo-search", "makati");
  await app.waitFor("id:pp-rdo-opt-047");
  await app.setLifecycle("inactive");
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  expect(app.value("id:pp-rdo-search")).toBe("050 — South Makati");
  expect(line(app, "rdo")).toBe("rdo: 050 | picks: 0 | opens: 1");
  await app.setLifecycle("active");
  // Back in the window the box shows what it holds, and says why the words went.
  await app.waitFor("text:“makati” fits 2 choices. Pick one from the list. The box kept 050 — South Makati.");
});

test("desktop: the focus moved away with no press (the form's focus()) closes the list and puts the last pick back with a sentence", async () => {
  const app = await probe({ rdo: "050" }, DESKTOP);
  await app.clear("id:pp-rdo-search");
  await app.type("id:pp-rdo-search", "makati");
  await app.waitFor("id:pp-rdo-opt-047");
  expect(app.shows("id:pp-rdo-list")).toBe(true);
  // The page's own focus() into another box (a pill step, Queue's jump): no press, so the list's onPressOutside hears nothing.
  await app.host.send("focus-other");
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  expect(app.value("id:pp-rdo-search")).toBe("050 — South Makati");
  expect(line(app, "rdo")).toBe("rdo: 050 | picks: 0 | opens: 1");
  // Two choices fit "makati": not taken, as Tab would not take it (resolveTyped), and the line under the box says so.
  await app.waitFor("text:“makati” fits 2 choices. Pick one from the list. The box kept 050 — South Makati.");
  expect(app.shows("id:pp-rdo-note")).toBe(true);
});

test("desktop: a row pressed while the list is open picks it, and the keys stay in the box", async () => {
  const app = await probe({ rdo: "050" }, DESKTOP);
  await app.clear("id:pp-rdo-search");
  await app.type("id:pp-rdo-search", "makati");
  await app.waitFor("id:pp-rdo-opt-047");
  // "makati" alone names no one choice: had the leaving been settled, the box would go back to 050 with a sentence.
  await app.press("id:pp-rdo-opt-047");
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  await app.wait(50);
  expect(line(app, "rdo")).toBe("rdo: 047 | picks: 1 | opens: 1");
  expect(app.value("id:pp-rdo-search")).toBe("047 — East Makati");
  expect(app.shows("id:pp-rdo-note")).toBe(false);
  // The row took no focus (keepsFocus): Down goes to the box, which opens its list again.
  await app.keyboard.press("down");
  await app.waitFor("id:pp-rdo-list");
  await app.keyboard.press("escape");
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  // The chevron closes the list it opened, and takes no focus either.
  await app.press("id:pp-rdo-trigger");
  await app.waitFor("id:pp-rdo-list");
  await app.press("id:pp-rdo-trigger");
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  await app.wait(50);
  expect(app.shows("id:pp-rdo-list")).toBe(false);
  expect(line(app, "rdo")).toBe("rdo: 047 | picks: 1 | opens: 3");
  await app.keyboard.press("down");
  await app.waitFor("id:pp-rdo-list");
});

test("desktop: a press whose row goes away under it never keeps a later leaving from settling", async () => {
  const app = await probe({ rdo: "050" }, DESKTOP);
  await app.clear("id:pp-rdo-search");
  await app.type("id:pp-rdo-search", "makati");
  await app.waitFor("id:pp-rdo-opt-047");
  // A finger goes down on a row and stays down.
  await app.dragAndHold("id:pp-rdo-opt-047", { y: 1 });
  // The keys come back to the box and change the words: the row the finger is on goes away, so its press-up never comes.
  await app.type("id:pp-rdo-search", "zzz");
  await app.waitFor("id:pp-rdo-opt-047", { state: "gone" });
  await app.drop();
  // A later leaving with the list open (the form's focus(), no press): the box settles at once.
  await app.clear("id:pp-rdo-search");
  await app.type("id:pp-rdo-search", "makati");
  await app.waitFor("id:pp-rdo-opt-047");
  await app.host.send("focus-other");
  await app.waitFor("id:pp-rdo-list", { state: "gone", timeout: 1000 });
  expect(app.value("id:pp-rdo-search")).toBe("050 — South Makati");
  expect(line(app, "rdo")).toBe("rdo: 050 | picks: 0 | opens: 1");
});

test("desktop: a box that goes away under a press picks and settles nothing afterwards", async () => {
  const app = await probe({ rdo: "050" }, { ...DESKTOP, clock: {} });
  await app.clear("id:pp-rdo-search");
  // A code typed whole: settled, it would pick 047.
  await app.type("id:pp-rdo-search", "047");
  await app.waitFor("id:pp-rdo-opt-047");
  await app.dragAndHold("id:pp-rdo-opt-047", { y: 1 });
  // The pick goes away (its page, the lock) before the press ends.
  await app.host.send("hide-rdo");
  await app.waitFor("id:pp-rdo-search", { state: "gone" });
  await app.wait(1_000);
  await app.drop();
  await app.wait(100);
  expect(line(app, "rdo")).toBe("rdo: 050 | picks: 0 | opens: 1");
});

test("desktop: a slow press held three seconds still picks its row", async () => {
  const app = await probe({ rdo: "050" }, { ...DESKTOP, clock: {} });
  await app.clear("id:pp-rdo-search");
  await app.type("id:pp-rdo-search", "makati");
  await app.waitFor("id:pp-rdo-opt-047");
  await app.dragAndHold("id:pp-rdo-opt-047", {});
  await app.wait(3_000);
  // Still open under the finger: no backstop cut the press short.
  expect(app.shows("id:pp-rdo-opt-047")).toBe(true);
  await app.drop();
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  await app.wait(50);
  expect(line(app, "rdo")).toBe("rdo: 047 | picks: 1 | opens: 1");
  expect(app.value("id:pp-rdo-search")).toBe("047 — East Makati");
});

test("desktop: the clear row clears a chosen value from the whole list", async () => {
  const app = await probe({ rdo: "050" }, DESKTOP);
  await app.press("id:pp-rdo-trigger");
  // The list opens on the current choice; the clear row heads it.
  await app.waitFor("id:pp-rdo-opt-050");
  await scrollTo(app, "id:pp-rdo-list", -600, "id:pp-rdo-opt-");
  await app.press("id:pp-rdo-opt-");
  await app.waitFor("id:pp-rdo-list", { state: "gone" });
  expect(line(app, "rdo")).toBe("rdo:  | picks: 1 | opens: 1");
  expect(app.value("id:pp-rdo-search")).toBe("");
});
