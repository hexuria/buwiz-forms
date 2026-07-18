import { describe, expect, it, vi } from "vitest";
import {
  assertBundledPrintableFontsReady,
  type PrintableFontFaceSetLike
} from "../src/printFonts";

const REQUIRED_FACE_REQUESTS = [
  ["normal 400 16px \"eBIRForms Arimo\"", "BIR Forms 2551Q"],
  ["normal 400 16px \"eBIRForms Arimo\"", "\u0100"],
  ["normal 700 16px \"eBIRForms Arimo\"", "BIR Forms 2551Q"],
  ["normal 700 16px \"eBIRForms Arimo\"", "\u0100"],
  ["italic 400 16px \"eBIRForms Arimo\"", "BIR Forms 2551Q"],
  ["italic 400 16px \"eBIRForms Arimo\"", "\u0100"],
  ["italic 700 16px \"eBIRForms Arimo\"", "BIR Forms 2551Q"],
  ["italic 700 16px \"eBIRForms Arimo\"", "\u0100"],
  ["normal 400 16px \"eBIRForms Roboto Condensed\"", "BIR Forms 2551Q"],
  ["italic 400 16px \"eBIRForms Roboto Condensed\"", "BIR Forms 2551Q"]
] as const;

function loadedFontSet(
  overrides: Partial<PrintableFontFaceSetLike> = {}
): PrintableFontFaceSetLike {
  return {
    ready: Promise.resolve(),
    load: vi.fn(async (descriptor: string) => [
      {
        family: descriptor.includes("Roboto Condensed")
          ? "eBIRForms Roboto Condensed"
          : "eBIRForms Arimo",
        status: "loaded",
        style: descriptor.startsWith("italic") ? "italic" : "normal",
        weight: descriptor.includes("Roboto Condensed") ? "100 900" : "400 700"
      }
    ]),
    check: vi.fn(() => true),
    ...overrides
  };
}

describe("bundled printable font readiness", () => {
  it("loads and checks every required bundled printable style and subset", async () => {
    const fontFaceSet = loadedFontSet();

    await expect(
      assertBundledPrintableFontsReady(fontFaceSet)
    ).resolves.toBeUndefined();

    expect(fontFaceSet.load).toHaveBeenCalledTimes(REQUIRED_FACE_REQUESTS.length);
    expect(fontFaceSet.check).toHaveBeenCalledTimes(REQUIRED_FACE_REQUESTS.length);
    for (const [descriptor, probe] of REQUIRED_FACE_REQUESTS) {
      expect(fontFaceSet.load).toHaveBeenCalledWith(descriptor, probe);
      expect(fontFaceSet.check).toHaveBeenCalledWith(descriptor, probe);
    }
  });

  it("fails closed when the font request rejects", async () => {
    const fontFaceSet = loadedFontSet({
      load: vi.fn(async () => {
        throw new Error("font request failed");
      })
    });

    await expect(assertBundledPrintableFontsReady(fontFaceSet)).rejects.toThrow(
      "Required bundled printable font face is unavailable"
    );
  });

  it("does not accept fallback-only load and check results", async () => {
    const noDeclaredFace = loadedFontSet({ load: vi.fn(async () => []) });
    await expect(
      assertBundledPrintableFontsReady(noDeclaredFace)
    ).rejects.toThrow("Required bundled printable font face is unavailable");

    const failedFace = loadedFontSet({
      load: vi.fn(async () => [
        {
          family: "eBIRForms Arimo",
          status: "error",
          style: "normal",
          weight: "400 700"
        }
      ])
    });
    await expect(assertBundledPrintableFontsReady(failedFace)).rejects.toThrow(
      "Required bundled printable font face is unavailable"
    );

    const uncheckedFace = loadedFontSet({ check: vi.fn(() => false) });
    await expect(
      assertBundledPrintableFontsReady(uncheckedFace)
    ).rejects.toThrow("Required bundled printable font face is unavailable");
  });

  it("rejects browser-synthesized style and weight fallbacks", async () => {
    const normalOnly = loadedFontSet({
      load: vi.fn(async () => [
        {
          family: "eBIRForms Arimo",
          status: "loaded",
          style: "normal",
          weight: "400"
        }
      ])
    });
    await expect(assertBundledPrintableFontsReady(normalOnly)).rejects.toThrow(
      "Required bundled printable font face is unavailable"
    );

    const wrongFamily = loadedFontSet({
      load: vi.fn(async (descriptor: string) => [
        {
          family: "Arial",
          status: "loaded",
          style: descriptor.startsWith("italic") ? "italic" : "normal",
          weight: "400 700"
        }
      ])
    });
    await expect(assertBundledPrintableFontsReady(wrongFamily)).rejects.toThrow(
      "Required bundled printable font face is unavailable"
    );
  });

  it("fails closed when the Font Loading API is absent or never settles", async () => {
    await expect(assertBundledPrintableFontsReady(undefined)).rejects.toThrow(
      "Printable font loading API is unavailable"
    );

    const rejectedReady = loadedFontSet({
      ready: Promise.reject(new Error("font set failed"))
    });
    await expect(
      assertBundledPrintableFontsReady(rejectedReady)
    ).rejects.toThrow("Bundled printable font set failed to settle");

    const pendingReady = loadedFontSet({
      ready: new Promise(() => undefined)
    });
    await expect(
      assertBundledPrintableFontsReady(pendingReady, 5)
    ).rejects.toThrow("Bundled printable font set failed to settle");

    const pendingFace = loadedFontSet({
      load: vi.fn(
        () => new Promise<readonly never[]>(() => undefined)
      )
    });
    await expect(
      assertBundledPrintableFontsReady(pendingFace, 5)
    ).rejects.toThrow("Required bundled printable font face is unavailable");
  });
});
