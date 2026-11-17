// What a screen imports from the frame: navigation, the open taxpayer, toasts. Never a shell component.
//
//   import { app, go, back, openReturn, openProfile, toast } from "../../shell";
//   go("filings", { preset: "overdue" });             // open a view (views that need a taxpayer fall back to Home)
//   openReturn({ tin: b.tin, code: "2551Q", year: 2026, period: 1 });
//   const name = () => app.taxpayer()?.name;          // accessors read inside JSX or a closure stay live
//   store.taxYears(app.branch()?.tin);                // the cached reads
export { app, back, forwardOne as forward, go, openBranch, openProfile, openReturn, openTaxpayer, replace, toast, type Notification, type OpenReturnOptions, type PanelName, type Prefs, type Session } from "./app";
export { contentBox, contentColumn, ROUTES, TABS, type Route, type RouteMeta, type ViewLayout, type ViewName, type ViewParams } from "./routes";
export { ALREADY_SENT, cancelQueue, cancelQueueMessage, isAlreadySent } from "./queue";
export { store, type Store } from "./store";
export { holdHotkey, hotkeyStatus } from "./desktop/hotkey_state";
export { refreshToday, today, todayIso } from "./clock";
