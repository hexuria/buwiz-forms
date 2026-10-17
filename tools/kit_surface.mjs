// The real kit's method surface for the JS harnesses: every method name
// app/shell/gpui-kit.d.ts declares. A mock element answers only these, so
// a call the real kit lacks is "not a function" in a harness, as it is in
// the app, instead of a mock quietly accepting it.
import fs from "node:fs";

export function kitMethods(root) {
  const dts = fs.readFileSync(new URL("app/shell/gpui-kit.d.ts", root), "utf8");
  const names = new Set();
  for (const m of dts.matchAll(/^\s+([a-z_][a-zA-Z0-9_]*)\s*[<(?]/gm)) names.add(m[1]);
  return names;
}

/**
 * A mock element (a `div()` and what styling it returns) that knows only
 * the kit's methods. `onCall(name, args, el)` sees each call (for `child`
 * mounting and `when` callbacks).
 */
export function strictElement(methods, onCall) {
  const el = new Proxy({}, {
    get(_o, k) {
      if (typeof k !== "string" || !methods.has(k)) return undefined;
      return (...args) => {
        onCall?.(k, args, el);
        return el;
      };
    },
  });
  return el;
}
