import { signal } from "nsh";

// The x a box's cell was laid out at, keyed by the box's form key, reported from rows.tsx's
// onLayout. X does not move with scrolling, so a row's report stays true after it recycles out
// of view. The FormPageAny fixture republishes the entries as probe text; a page test compares
// columns across rows (the penalty rows' boxes must sit exactly where the rows above them sit).
const xs = signal<Record<string, number>>({});

export function reportBoxX(key: string, x: number) {
  const now = xs();
  if (Math.abs((now[key] ?? -1) - x) < 0.5) return;
  xs.set({ ...now, [key]: x });
}

export function boxXs(): Record<string, number> {
  return xs();
}
