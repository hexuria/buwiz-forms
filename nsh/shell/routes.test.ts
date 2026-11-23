// The content column of a page by the old rule (app/main.js layout(), the old shell's ui.js SIZE): the cap of 880 (1120 for the wide
// views, none for Filings) INCLUDES the surface's padding of 32 (16 under 640 points of surface). Pure.
import { expect, test } from "nsh/test";
import { contentBox, contentColumn } from "./routes";

test("at 1440 wide the page column is 816, not 880: the cap includes its padding", () => {
  // Window 1440, sidebar docked, no panel: the surface is 1440 - 248 - 8 - 2 = 1182.
  expect(contentBox("page", 1182)).toEqual({ pad: 32, maxWidth: 816 });
  expect(contentColumn("page", 1182)).toBe(816);
  expect(contentBox("wide", 1182)).toEqual({ pad: 32, maxWidth: 1056 });
  expect(contentColumn("wide", 1182)).toBe(1056);
  expect(contentBox("full", 1182).maxWidth).toBeUndefined();
  expect(contentColumn("full", 1182)).toBe(1118);
});

test("a narrower surface takes the surface less its padding; under 640 the padding is 16", () => {
  expect(contentColumn("page", 700)).toBe(636);
  expect(contentBox("page", 639)).toEqual({ pad: 16, maxWidth: 848 });
  expect(contentColumn("page", 639)).toBe(607);
  expect(contentColumn("page", 240)).toBe(208);
  expect(contentColumn("wide", 240)).toBe(208);
  // The old floor of 200.
  expect(contentColumn("page", 100)).toBe(200);
});
