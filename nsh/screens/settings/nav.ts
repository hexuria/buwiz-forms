// Where Settings is: the search text and the section last opened. Module state with one owner (this file), kept for
// the run like the old app's settingsSearch and settingsSection, so a return to Settings finds the page as it was left.
import { signal } from "nsh";

const query = signal("");
const section = signal("general");

/** The search and the section Settings shows. */
export const nav = {
  query: (): string => query(),
  setQuery: (text: string): void => query.set(text),
  section: (): string => section(),
  setSection: (id: string): void => section.set(id),
};
