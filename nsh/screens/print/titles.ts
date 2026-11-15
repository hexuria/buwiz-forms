// What the preview window says and does with the form page, in pure functions (no UI): its id and title, which return it
// belongs to, and the script the toolbar runs in the sheet's page (the desk color and the zoom).
import type { FormTarget } from "../form/FormPage";

/** What a device with no second window says (a phone). */
export const UNAVAILABLE = "Print preview is not available on this device.";

/**
 * The window id of a preview: the form code and the return it was asked on (`formIdentity`: form, TIN, year, period). Another return of
 * the same form is another window, so it never lands in one that is closing for the last. A preview asked off a form page has the code only.
 */
export function windowId(key: string, identity?: string | null): string {
  return identity ? `print:${key}:${identity.replace(/[^A-Za-z0-9]+/g, "-")}` : `print:${key}`;
}

/**
 * The window title: the host function's "BIR Form 2551Q — Juana Santos" with the period after the code
 * ("BIR Form 2551Q · Q1 2026 — Juana Santos"). A title without a name, or no period, keeps what it has.
 */
export function previewTitle(hostTitle: string, period?: string | null): string {
  if (!period) return hostTitle;
  const parts = /^(.*?)\s+—\s+(.*)$/.exec(hostTitle);
  return parts ? `${parts[1]} · ${period} — ${parts[2]}` : `${hostTitle} · ${period}`;
}

/** What the toolbar says: "A4 · 4 pages · Folio sheet at 88%". */
export function pageSummary(info: { paper: string; pages: number; sheet: string; zoom: number }): string {
  const pages = `${info.pages} ${info.pages === 1 ? "page" : "pages"}`;
  const scaled = info.zoom < 0.995 ? ` · ${info.sheet} sheet at ${Math.round(info.zoom * 100)}%` : "";
  return `${info.paper} · ${pages}${scaled}`;
}

/** The part of a route this module reads (the frame's `Route`). */
export interface RouteLike {
  view: string;
  params: Record<string, unknown>;
}

/**
 * Which return a route shows, as far as a preview cares: code, taxpayer, year, period and whether it is a Form library
 * sample. Null when the route is not a form page. Moving to another return (or leaving the form) ends the preview;
 * saving an amendment for the first time (the same return, another mode and key) does not.
 */
export function formIdentity(route: RouteLike): string | null {
  if (route.view !== "form") return null;
  const target = route.params.target as FormTarget | undefined;
  if (!target || typeof target !== "object") return null;
  return [target.formCode, target.tin, target.year, target.period, target.mode === "sample" ? "sample" : "return"].join("|");
}

/** The two ways to size the sheet. */
export type ZoomMode = "fit" | "100";

/**
 * The script that paints the page's desk (the grey behind the sheets) in `desk` and sizes the sheets: "100" is their size,
 * "fit" the page's width less 48 (24 each side), between 0.3 and 2.5 times. The sheets themselves stay white; their
 * shadow is lighter on a light desk.
 */
export function chromeScript(mode: ZoomMode, desk: string, dark: boolean): string {
  const shadow = dark ? "0 6px 22px rgba(0,0,0,.6)" : "0 1px 3px rgba(0,0,0,.2),0 6px 18px rgba(0,0,0,.16)";
  const css = `html{background:${desk} !important}.sheet{box-shadow:${shadow} !important}`;
  return (
    '(function(m,css){var d=document.documentElement;var s=document.getElementById("preview-chrome");' +
    'if(!s){s=document.createElement("style");s.id="preview-chrome";document.head.appendChild(s);}' +
    's.textContent=css;d.style.zoom="1";var sh=document.querySelector(".sheet");' +
    'if(m==="fit"&&sh){var w=sh.getBoundingClientRect().width;if(w>0){d.style.zoom=String(Math.max(0.3,Math.min(2.5,(window.innerWidth-48)/w)));}}' +
    'return "chrome";})(' +
    JSON.stringify(mode) +
    "," +
    JSON.stringify(css) +
    ");"
  );
}

/** The file name of a path, for "Saved Juana.pdf". */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}
