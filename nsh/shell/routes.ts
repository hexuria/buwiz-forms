// The views the frame can show: one table, no UI. A screen agent adds a view here (name, label, icon, whether it needs a
// taxpayer, how the frame lays it out) and maps it to its component in views.tsx; nothing else in the shell changes.
import type { IconName } from "../ui";

/**
 * How the frame places a view in the main surface.
 * page: the frame's own scroll, content column 880 wide, centred.
 * wide: the same, 1120 wide (the form library).
 * full: the same without a cap (the Filings board takes the whole pane).
 * self: the view fills the pane and scrolls by itself (the form page has its own scroll areas, a 920-wide column and a pill).
 */
export type ViewLayout = "page" | "wide" | "full" | "self";

/** What the shell knows of a view. */
export interface RouteMeta {
  /** Shown in the palette ("Go to") and the narrow header. */
  label: string;
  icon: IconName;
  /** Needs a taxpayer: with none the frame shows Home instead. */
  needsTaxpayer: boolean;
  layout: ViewLayout;
  /** Only where the dev environment is on (EBIR_TEST_ENV: every run, shot and test recipe sets it). */
  dev?: boolean;
}

const TABLE = {
  home: { label: "Home", icon: "house", needsTaxpayer: false, layout: "page" },
  library: { label: "Form library", icon: "file-text", needsTaxpayer: false, layout: "wide" },
  calendar: { label: "Calendar", icon: "calendar", needsTaxpayer: false, layout: "page" },
  filings: { label: "Filings", icon: "history", needsTaxpayer: false, layout: "full" },
  settings: { label: "Settings", icon: "settings", needsTaxpayer: false, layout: "page" },
  profile: { label: "Tax profile", icon: "user", needsTaxpayer: true, layout: "page" },
  forms: { label: "Forms", icon: "file-text", needsTaxpayer: true, layout: "page" },
  templates: { label: "Templates", icon: "layout-template", needsTaxpayer: true, layout: "page" },
  new_taxpayer: { label: "New taxpayer", icon: "user", needsTaxpayer: false, layout: "page" },
  new_branch: { label: "New branch", icon: "git-branch", needsTaxpayer: true, layout: "page" },
  form: { label: "Return", icon: "file-text", needsTaxpayer: true, layout: "self" },
  kit: { label: "UI kit", icon: "layout-template", needsTaxpayer: false, layout: "self", dev: true },
  theme: { label: "Theme page", icon: "monitor", needsTaxpayer: false, layout: "self", dev: true },
} as const satisfies Record<string, RouteMeta>;

/** A view's name. */
export type ViewName = keyof typeof TABLE;

/** Every view by name. */
export const ROUTES: Readonly<Record<ViewName, RouteMeta>> = TABLE;

/** What a view is opened with (a Home tile's Filings preset, a settings section, the form to open ...). */
export type ViewParams = Record<string, unknown>;

/** Where the app is: a view and its params. `seq` changes with every move, so a view is made again only when the person moved. */
export interface Route {
  view: ViewName;
  params: ViewParams;
  seq: number;
}

/** The four bottom tabs of the narrow layout, and the view each opens. */
export const TABS = [
  { view: "home", label: "Home", icon: "house" },
  { view: "library", label: "Forms", icon: "file-text" },
  { view: "filings", label: "Filings", icon: "history" },
  { view: "calendar", label: "Calendar", icon: "calendar" },
] as const satisfies readonly { view: ViewName; label: string; icon: IconName }[];

/** True for a name of ROUTES. */
export function isViewName(name: string): name is ViewName {
  return Object.prototype.hasOwnProperty.call(ROUTES, name);
}
