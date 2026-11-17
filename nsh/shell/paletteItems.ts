// What the command palette lists (old main.js paletteItems): Taxpayers, Branches, Go to, Forms, Actions. Typing narrows
// them; digits match a TIN (nine digits name a taxpayer, ten or more a branch). Pure over the app state: Palette.tsx draws it.
import { computed, keymap, signal, untracked } from "nsh";
import { doorStatus } from "../bir";
import { formUi } from "../screens/form/formUi";
import type { IconName } from "../ui";
import { app } from "./app";
import { ROUTES, type ViewName } from "./routes";
import { store } from "./store";

/** The palette's group headings, in the order they are listed. */
export type PaletteGroup = "Taxpayers" | "Branches" | "Go to" | "Forms" | "Actions";

/** One row of the palette. */
export interface PaletteItem {
  group: PaletteGroup;
  label: string;
  /** A TIN or a shortcut, shown at the end of the row. */
  hint?: string;
  icon?: IconName;
  /** A taxpayer's avatar (name and TIN root) instead of an icon. */
  avatar?: { name: string; root: string };
  /** Matched by digits of its TIN: kept whatever its label says. */
  match?: boolean;
  run: () => void;
}

/** The pages "Go to" offers, in order (the dev views follow, where the dev environment is on). */
const PAGES: ViewName[] = ["home", "library", "calendar", "filings", "profile", "forms", "templates", "settings"];

/** The forms the palette starts: label and code, from the form library (screen_forms_index). */
interface FormRow {
  code: string;
  label: string;
}

/**
 * The palette's index of forms. Building it is a host call that makes the whole form library (about a second in a debug build),
 * so it is made when the palette opens (Palette.tsx calls indexPalette as it mounts) and not by the first typed letter, which
 * used to hold the list unfiltered for that second. The library never changes while the app runs, so what was built stays
 * (store.bump() drops the store's copy at every save; this one is kept, as the store keeps the RDO and ZIP lists).
 */
const formRows = signal<FormRow[] | null>(null);

function buildFormRows(): FormRow[] {
  return store.screenIndex().map((f) => ({ code: f.c, label: `${f.c} · ${f.t}` }));
}

/** Builds the form index if it is not built yet (a read that came back empty is tried again at the next open). */
export function indexPalette(): void {
  if (untracked(formRows) !== null) return;
  const rows = buildFormRows();
  if (rows.length) formRows.set(rows);
}

/** The rows for the typed text, in group order. */
export const paletteItems = computed<PaletteItem[]>(() => {
  const q = app.paletteQuery().trim().toLowerCase();
  const fresh = app.isFresh();
  const items: PaletteItem[] = [];
  // A TIN typed as digits, with or without dashes: 10 or more digits name a branch (root + branch code).
  const digits = q.replace(/[\s-]/g, "");
  const byTin = /^\d+$/.test(digits);
  const current = app.taxpayer();
  for (const t of app.taxpayers()) {
    if (byTin && digits.length > 9) {
      for (const b of t.branches) {
        if (!b.tin.startsWith(digits)) continue;
        items.push({
          group: "Taxpayers",
          label: t.branches.length > 1 ? `${t.name} · ${b.name}` : t.name,
          hint: b.tin_display,
          avatar: { name: t.name, root: t.tin_root },
          match: true,
          run: () => app.openBranch(t.tin_root, b.code),
        });
      }
      continue;
    }
    items.push({
      group: "Taxpayers",
      label: t.name,
      hint: t.tin_display,
      avatar: { name: t.name, root: t.tin_root },
      match: byTin && t.tin_root.startsWith(digits),
      run: () => app.openTaxpayer(t.tin_root),
    });
  }
  // The open taxpayer's branches are one step away.
  if (current && current.branches.length > 1 && !byTin) {
    for (const b of current.branches) {
      items.push({ group: "Branches", label: b.name, hint: b.tin_display, icon: "git-branch", run: () => app.openBranch(current.tin_root, b.code) });
    }
  }
  const dev = doorStatus().testEnvironment;
  const pages = [...PAGES, ...(Object.keys(ROUTES) as ViewName[]).filter((v) => ROUTES[v].dev && dev)];
  for (const view of pages) {
    const meta: { label: string; icon: IconName; needsTaxpayer: boolean } = ROUTES[view];
    if (fresh && meta.needsTaxpayer) continue;
    items.push({ group: "Go to", label: meta.label, icon: meta.icon, run: () => app.go(view) });
  }
  if (q) {
    // The index built at open; a palette read without opening (a test, a hook) builds it here, once.
    for (const f of formRows() ?? buildFormRows()) {
      items.push({ group: "Forms", label: f.label, icon: "file-text", run: () => app.startNewReturn(f.code) });
    }
  }
  // The open form's pill steps (old main.js: only while a form is open and shows its pill).
  if (app.view() === "form" && !fresh && formUi.canStep()) {
    items.push({ group: "Actions", label: "Next to do", icon: "chevron-right", hint: keymap.label("mod+right"), run: () => formUi.step(1) });
    items.push({ group: "Actions", label: "Previous to do", icon: "chevron-left", hint: keymap.label("mod+left"), run: () => formUi.step(-1) });
  }
  items.push({ group: "Actions", label: "Toggle sidebar", icon: "panel-left", hint: keymap.label("mod+b"), run: app.toggleSidebar });
  items.push({
    group: "Actions",
    label: app.panel() === "profile" ? "Hide tax profile" : "Show tax profile",
    icon: "panel-right",
    hint: keymap.label("mod+shift+b"),
    run: () => app.togglePanel("profile"),
  });
  items.push({
    group: "Actions",
    label: app.panel() === "notifs" ? "Hide notifications" : "Show notifications",
    icon: "bell",
    hint: keymap.label("mod+shift+n"),
    run: () => app.togglePanel("notifs"),
  });
  items.push({ group: "Actions", label: "Change theme", icon: "moon", hint: keymap.label("mod+t"), run: app.cycleTheme });
  if (!q) return items;
  return items.filter((x) => x.match || `${x.label} ${x.hint ?? ""}`.toLowerCase().includes(q));
});

/** The highlighted row, kept inside the list. */
export function paletteRow(): number {
  return Math.min(app.paletteIndex(), Math.max(0, paletteItems().length - 1));
}

/** Moves the highlight by delta, wrapping at the ends. */
export function movePalette(delta: number): void {
  const n = paletteItems().length;
  if (n) app.setPaletteIndex((paletteRow() + delta + n) % n);
}

/** Closes the palette and runs row `index` (Enter, or a press). */
export function runPaletteItem(index: number): void {
  const item = paletteItems()[index];
  app.closeOverlay();
  item?.run();
}
