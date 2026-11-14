// Which component shows for each view. A screen agent replaces the Placeholder of its view here with the real screen (one
// line), after adding the view to routes.ts if it is new. A view is made once per move (the route's seq), not per param change.
import type { JSX } from "nsh";
import { FormPage, type FormTarget } from "../screens/form/FormPage";
import { Placeholder } from "../screens/placeholder/Placeholder";
import { app } from "./app";
import type { Route } from "./routes";

/** The form page for the route's target; the frame follows what the page says (a sample became its return, a save). */
function FormView(props: { target: FormTarget }) {
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
      toast={(message, o) => app.toast(message, o?.icon)}
    />
  );
}

/** The screen for a route. */
export function renderView(route: Route): JSX.Element {
  switch (route.view) {
    case "form":
      return <FormView target={route.params.target as FormTarget} />;
    case "library":
      return <Placeholder view="library" title="Form library" coming="wave 3 (C2)" old="Every form with a search box and frequency filter; a form opens a sample, or the start-a-return dialog." />;
    case "calendar":
      return <Placeholder view="calendar" title="Calendar" coming="wave 3 (C3)" old="Deadlines as a list or a month grid, with a filter bar." />;
    case "filings":
      return <Placeholder view="filings" title="Filings" coming="wave 3 (C4)" old="Returns by status: not started, draft, queued, filed." />;
    case "settings":
      return <Placeholder view="settings" title="Settings" coming="wave 4 (D1)" old="General, security, data, jobs and logs." />;
    case "profile":
      return <Placeholder view="profile" title="Tax profile" coming="wave 3 (C5)" old="The taxpayer's details, branches and tax years." withTaxpayer />;
    case "forms":
      return <Placeholder view="forms" title="Forms" coming="wave 3 (C5)" old="The forms this branch files in the tax year." withTaxpayer />;
    case "templates":
      return <Placeholder view="templates" title="Templates" coming="wave 3 (C5)" old="Saved answers per form." withTaxpayer />;
    case "new_taxpayer":
      return <Placeholder view="new_taxpayer" title="New taxpayer" coming="wave 3 (C5)" old="The form to add a taxpayer and its head office." />;
    case "new_branch":
      return <Placeholder view="new_branch" title="New branch" coming="wave 3 (C5)" old="The form to add a branch to the taxpayer." withTaxpayer />;
    default:
      return <Placeholder view="home" title="Home" coming="wave 3 (C1)" old="The dashboard: what is overdue, due this week, drafts, waiting to send, and the next 30 days." />;
  }
}
