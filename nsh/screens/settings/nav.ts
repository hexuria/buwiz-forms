// Where Settings is: the search text and the section last opened. Module state with one owner (this file), kept for
// the run like the old app's settingsSearch and settingsSection, so a return to Settings finds the page as it was left.
import { signal } from "nsh";
import { app } from "../../shell";

const query = signal("");
const section = signal("general");
// The next session starts from the defaults (a lock, a sign-out or another taxpayer signing in): no search, the first section.
app.onSessionChange(() => {
  query.set("");
  section.set("general");
});

/** The search and the section Settings shows. */
export const nav = {
  query: (): string => query(),
  setQuery: (text: string): void => query.set(text),
  section: (): string => section(),
  setSection: (id: string): void => section.set(id),
};
