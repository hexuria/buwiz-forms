// The quit and hide keys' storage (app_keys.ts): a change made before the stored choices came back is applied over them, never instead
// of them, so a late Storage read can neither undo the change nor have the change wipe the keys it did not touch. A plain test with a
// stand-in Storage whose read answers when the test says.
import { expect, test } from "nsh/test";
import { closePrefs, CLOSE_PREFS_KEY, loadClosePrefs, setAppKey, setCloseChoice, useClosePrefsStorage } from "./app_keys";

/** A Storage whose read waits for `answer`, and the writes it got. */
function slowStorage() {
  let answer: (raw: string | null) => void = () => {};
  const read = new Promise<string | null>((done) => (answer = done));
  const writes: string[] = [];
  return {
    store: {
      getItem: (key: string) => (key === CLOSE_PREFS_KEY ? read : Promise.resolve(null)),
      setItem: async (_key: string, value: string) => {
        writes.push(value);
      },
    },
    answer: (raw: string | null) => answer(raw),
    writes,
  };
}

test("a key set before the stored ones came back keeps the stored others, and is what gets written", async () => {
  const s = slowStorage();
  useClosePrefsStorage(s.store);
  const loading = loadClosePrefs();
  // Settings changes the hide key while the read is still out: it shows at once, nothing is written yet.
  setAppKey("hide", "alt+cmd+h");
  expect(closePrefs().keys.hide).toBe("alt+cmd+h");
  expect(s.writes).toEqual([]);
  // The read comes back with a quit key and a close choice of the user's: they stay, the new hide key goes over them.
  s.answer(JSON.stringify({ quit: "alt+cmd+q", hide: "cmd+h", close: "quit" }));
  await loading;
  expect(closePrefs()).toEqual({ keys: { quit: "alt+cmd+q", hide: "alt+cmd+h" }, close: "quit" });
  expect(s.writes.map((w) => JSON.parse(w))).toEqual([{ quit: "alt+cmd+q", hide: "alt+cmd+h", close: "quit" }]);
  // After the read, a change is written at once, over what is there now.
  setCloseChoice("hide");
  expect(JSON.parse(s.writes[s.writes.length - 1] ?? "{}")).toEqual({ quit: "alt+cmd+q", hide: "alt+cmd+h", close: "hide" });
});

test("a change made before any read starts one, and still waits for it", async () => {
  const s = slowStorage();
  useClosePrefsStorage(s.store);
  setCloseChoice("hide");
  expect(s.writes).toEqual([]);
  s.answer(JSON.stringify({ quit: "ctrl+alt+q", hide: "", close: "ask" }));
  await loadClosePrefs();
  expect(closePrefs()).toEqual({ keys: { quit: "ctrl+alt+q", hide: "" }, close: "hide" });
  expect(JSON.parse(s.writes[0] ?? "{}")).toEqual({ quit: "ctrl+alt+q", hide: "", close: "hide" });
});
