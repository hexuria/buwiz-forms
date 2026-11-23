// The Calendar: every deadline for the forms the taxpayers file, from the BIR tax calendar (the old shell's calendar_page.js).
// From 600 points: the filter bar with a List and a Month view. Under 600: a month grid with due dots and an Upcoming list.
// Holidays move deadlines only in the years the app ships a holiday list for (bir-core official_holidays: 2026 so far); a
// later year's deadlines move off weekends only, so one can show on a holiday until an update adds it.
import { app } from "../../shell";
import { NarrowCalendar } from "./Narrow";
import { WideCalendar } from "./Wide";

/** The Calendar screen; the layout follows the window (under 600 points the narrow one). */
export function CalendarPage() {
  return (
    <If when={app.layout.narrow()} else={<WideCalendar />}>
      <NarrowCalendar />
    </If>
  );
}
