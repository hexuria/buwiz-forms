import { describe, expect, test } from "nsh/test";
import { baseName, chromeScript, formIdentity, pageSummary, previewTitle, windowId } from "./titles";

describe("window title", () => {
  test("the period goes after the form code", () => {
    expect(previewTitle("BIR Form 2551Q — Juana Santos", "Q1 2026")).toBe("BIR Form 2551Q · Q1 2026 — Juana Santos");
  });
  test("a title with no name, or no period, keeps what it has", () => {
    expect(previewTitle("BIR Form 2551Q", "Q1 2026")).toBe("BIR Form 2551Q · Q1 2026");
    expect(previewTitle("BIR Form 2551Q — Juana Santos", null)).toBe("BIR Form 2551Q — Juana Santos");
    expect(previewTitle("BIR Form 2551Q — Juana Santos", "")).toBe("BIR Form 2551Q — Juana Santos");
  });
  test("one window id per form code off a form page, per return on one", () => {
    expect(windowId("2551Q")).toBe("print:2551Q");
    expect(windowId("2551Q", null)).toBe("print:2551Q");
    const a = windowId("2551Q", "2551Q|00012345300000|2026|3|return");
    expect(a).toBe("print:2551Q:2551Q-00012345300000-2026-3-return");
    // Another period, another TIN, another year, a sample: each is its own window, so none reuses one that is closing for another.
    for (const other of ["2551Q|00012345300000|2026|2|return", "2551Q|00012345300001|2026|3|return", "2551Q|00012345300000|2025|3|return", "2551Q|00012345300000|2026|3|sample"]) {
      expect(windowId("2551Q", other)).not.toBe(a);
    }
  });
});

describe("page summary", () => {
  test("the paper, the pages, and the scale when the sheet was scaled down", () => {
    expect(pageSummary({ paper: "A4", pages: 4, sheet: "Folio", zoom: 0.8782 })).toBe("A4 · 4 pages · Folio sheet at 88%");
    expect(pageSummary({ paper: "A4", pages: 1, sheet: "A4", zoom: 1 })).toBe("A4 · 1 page");
  });
});

describe("which return a route shows", () => {
  const target = { formCode: "2551Q", tin: "00012345300000", year: 2026, period: 3, mode: "return" };
  test("a form page is its code, taxpayer, year and period", () => {
    expect(formIdentity({ view: "form", params: { target } })).toBe("2551Q|00012345300000|2026|3|return");
  });
  test("an amendment's first save (another mode and key) is the same return", () => {
    const amend = { ...target, mode: "amend" };
    const saved = { ...target, mode: "return", key: 5 };
    expect(formIdentity({ view: "form", params: { target: amend } })).toBe(formIdentity({ view: "form", params: { target: saved } }));
  });
  test("a sample is not the return", () => {
    expect(formIdentity({ view: "form", params: { target: { ...target, mode: "sample" } } })).not.toBe(
      formIdentity({ view: "form", params: { target } }),
    );
  });
  test("another period, another return; no form page, no return", () => {
    expect(formIdentity({ view: "form", params: { target: { ...target, period: 2 } } })).not.toBe(
      formIdentity({ view: "form", params: { target } }),
    );
    expect(formIdentity({ view: "home", params: {} })).toBeNull();
    expect(formIdentity({ view: "form", params: {} })).toBeNull();
  });
});

describe("the page script", () => {
  test("it paints the desk, never the sheet, and sizes by mode", () => {
    const fit = chromeScript("fit", "#e2e2e7", false);
    expect(fit).toContain('"fit"');
    expect(fit).toContain("html{background:#e2e2e7 !important}");
    expect(fit).not.toContain(".sheet{background");
    expect(fit).toContain("window.innerWidth-48");
    const full = chromeScript("100", "#0e0e10", true);
    expect(full).toContain('"100"');
    expect(full).toContain("rgba(0,0,0,.6)");
    // The first and last characters make one call (the page runs it as written).
    expect(fit.startsWith("(function(m,css){")).toBe(true);
    expect(fit.endsWith(");")).toBe(true);
  });
});

describe("saved file name", () => {
  test("the last part of a path", () => {
    expect(baseName("/Users/x/Documents/BIR Form 2551Q.pdf")).toBe("BIR Form 2551Q.pdf");
    expect(baseName("C:\\Users\\x\\a.pdf")).toBe("a.pdf");
    expect(baseName("a.pdf")).toBe("a.pdf");
  });
});
