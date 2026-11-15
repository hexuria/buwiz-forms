// Which component shows for each view. A screen agent replaces the Placeholder of its view here with the real screen (one
// line), after adding the view to routes.ts if it is new. A view is made once per move (the route's seq), not per param change.
import { cleanup, effect, type JSX, untracked } from "nsh";
import { CalendarPage } from "../screens/calendar/CalendarPage";
import { Filings } from "../screens/filings/Filings";
import { flushNow, FormPage, type FormTarget } from "../screens/form/FormPage";
import type { FormSession } from "../screens/form/session";
import { Home } from "../screens/home/Home";
import { Library } from "../screens/library/Library";
import { Placeholder } from "../screens/placeholder/Placeholder";
import { Settings } from "../screens/settings";
import { NewBranchPage, NewTaxpayerPage, TaxpayerPage } from "../screens/taxpayer";
import { dismissToast } from "../ui";
import { app } from "./app";
import type { Route } from "./routes";

/**
 * The form page for the route's target; the frame follows what the page says (a sample became its return, a save). What is typed
 * is saved before the app leaves the page: go, replace, back, forward and quit wait for flushNow (3 seconds at most). The page's
 * toasts come through whole, so a removal's Undo shows, and the page closes the toast again with its id.
 */
function FormView(props: { target: FormTarget }) {
  cleanup(
    app.beforeLeave(async () => {
      await flushNow();
    }),
  );
  // An amendment's first save (the Save amendment button, with nothing typed) is a stored return as much as one with edits, but the
  // page tells the frame of a save only when something changed: watch the save state, so the history stops that name the amendment
  // open the saved return, not a new amendment.
  const amending = props.target.mode === "amend";
  let settled = false;
  const watchAmendment = (s: FormSession) => {
    if (!amending) return;
    effect(() => {
      if (settled || s.saveState().state !== "saved") return;
      settled = true;
      untracked(() => app.retarget({ ...s.target(), mode: "return" }));
    });
  };
  return (
    <FormPage
      tin={props.target.tin}
      formCode={props.target.formCode}
      year={props.target.year}
      period={props.target.period}
      key={props.target.key}
      mode={props.target.mode}
      snapshotId={props.target.snapshotId}
      onClose={() => app.back()}
      onOpenReturn={(t) => app.retarget(t)}
      onOpenProfile={() => app.openProfile()}
      onSaved={(t) => app.bumpRecent({ tin: t.tin, code: t.formCode, year: t.year, period: t.period, returnKey: t.key })}
      toast={(message, o) => app.toast(message, o?.icon, o?.action)}
      dismissToast={dismissToast}
      onSession={watchAmendment}
    />
  );
}

/** The screen for a route. */
export function renderView(route: Route): JSX.Element {
  switch (route.view) {
    case "form":
      return <FormView target={route.params.target as FormTarget} />;
    case "library":
      return <Library />;
    case "calendar":
      return <CalendarPage />;
    case "filings":
      return <Filings />;
    case "settings":
      return <Settings />;
    case "profile":
    case "forms":
    case "templates":
      return <TaxpayerPage view={route.view} params={route.params} />;
    case "new_taxpayer":
      return <NewTaxpayerPage />;
    case "new_branch":
      return <NewBranchPage />;
    default:
      return <Home />;
  }
}
