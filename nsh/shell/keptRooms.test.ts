// kept_values_follow_the_room (§A.8): what the app keeps (selection, Recent, read notifications) is kept under the
// room's database path, so one room never reads another's values and each room keeps its own when it opens again
// (I4, shell/app.ts storageKey). The full switch — sign out, a second account, its own room, back — is the Rust
// side's (a_second_account_gets_an_empty_room); here the keying that makes it true.
import { expect, test } from "nsh/test";
import { storageKey } from "./app";

test("kept values are keyed per database path and follow their room", () => {
  const roomA = "/device/rooms/aaaa/bir_data.db";
  const roomB = "/device/rooms/bbbb/bir_data.db";
  // The same value under two rooms is two keys: a write in room A is invisible in room B...
  expect(storageKey("selection", roomA)).not.toBe(storageKey("selection", roomB));
  expect(storageKey("selection", roomA)).toBe("selection@" + roomA);
  // ...and when room A opens again its key is the same: its values are still its own.
  expect(storageKey("selection", roomA)).toBe(storageKey("selection", roomA));
  // An empty path (no room open) keeps nothing rather than sharing between rooms.
  expect(storageKey("selection", "")).toBeNull();
  // Prefs are the app's, not a room's: one key, no path.
  expect(storageKey("redesign.prefs", roomA)).toBe("redesign.prefs");
  expect(storageKey("redesign.prefs", "")).toBe("redesign.prefs");
});
