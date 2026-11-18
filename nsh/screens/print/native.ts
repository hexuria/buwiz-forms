// The one place the print preview reaches the Rust side: the `Print` module of `src/print.rs` (`ebirforms/native`).
// The preview is not a door function (it is not in `nsh/bir`): a window, a PDF and the system print panel are the
// desktop glue's, so its host (host.tsx) and its window (PreviewWindow.tsx) import `Print` from here and nowhere else.
import { Print } from "ebirforms/native";
import type { PdfProbe, PrintEvent, SheetImage } from "ebirforms/native";

export { Print };
export type { PdfProbe, PrintEvent, SheetImage };

/**
 * A path for Save as PDF that stands in for the save panel. Only a test sets it (a fixture); the Rust side ignores
 * it outside a test environment (EBIR_TEST_ENV), so the person is always asked in the real app.
 */
export const testing: { savePath: string | undefined } = { savePath: undefined };

/** What the window's buttons do: the real ones, or a test's. */
export interface PrintActions {
  /** Makes the PDF and shows the system print panel. */
  print(key: string): Promise<void>;
  /** Makes the PDF and saves it where the save panel says: the path, or null when cancelled. */
  savePdf(key: string): Promise<string | null>;
  /**
   * The sheets of the preview as pictures, where the app draws them itself (Windows and Linux: on the CPU, so the preview
   * needs no web view and no GPU). Absent where a web view shows the sheet's page (macOS, and a test's own actions).
   */
  sheets?(key: string): Promise<SheetImage[]>;
}

/** The real actions. */
export const REAL_ACTIONS: PrintActions = {
  print: (key) => Print.print(key),
  savePdf: (key) => Print.savePdf(key, testing.savePath ?? null),
  ...(Print.drawsSheets() ? { sheets: (key: string) => Print.sheets(key) } : {}),
};
