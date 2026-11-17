// What Home has picked under its strip: a tile (overdue, due in 7 days), a day of the strip, or nothing (old app: homePick and
// homeDay on the app). It outlives the view on purpose: the old app kept it while the app ran, so Back from an opened return
// shows the list you left. One owner: this file's signals; the view reads the accessors and calls the actions.
import { signal } from "nsh";
import type { HomePick } from "../../lib/dashboard";
import { app } from "../../shell";

const pick = signal<HomePick | null>(null);
const day = signal<string | null>(null);
// The next session starts from the defaults (a lock, a sign-out or another taxpayer signing in): nothing picked.
app.onSessionChange(() => {
  pick.set(null);
  day.set(null);
});

/** The picked tile or day (reactive). */
export const homePick = {
  /** "overdue" or "week" (a tile), "day" (a day of the strip), or null. */
  pick: (): HomePick | null => pick(),
  /** The picked day as YYYY-MM-DD while pick() is "day". */
  day: (): string | null => day(),
  /** A press on the Overdue or Due in 7 days tile: picks it, or clears it when it was the pick. */
  toggleTile(key: "overdue" | "week"): void {
    pick.set(pick() === key ? null : key);
  },
  /** A press on a day of the strip: picks it, or clears it when it was the pick. */
  toggleDay(iso: string): void {
    if (pick() === "day" && day() === iso) pick.set(null);
    else {
      pick.set("day");
      day.set(iso);
    }
  },
  /** "Show all": back to every open deadline of the next 30 days. */
  clear(): void {
    pick.set(null);
  },
};
