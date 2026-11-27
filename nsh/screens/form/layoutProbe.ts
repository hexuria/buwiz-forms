// Test seam (gated as Pick.tsx's watchSheetFrames is): unless a test attached with
// watchBoxFrames(true), nothing is registered, no onLayout is attached and release
// builds pay nothing. The form fixture (FormPageAny) republishes the frames as
// invisible text so a test reads them; measure() polling for inputs rides the
// fixture's probe interval.
import { signal } from "nsh";

export interface BoxFrame {
  x: number;
  width: number;
}

const frames = signal<Record<string, BoxFrame>>({});
let active = false;
const measurers = new Map<string, () => Promise<{ pagePosition: { x: number }; bounds: { size: { width: number } } } | null>>();

/** Whether a test attached: rows and controls attach their probe handlers only then. */
export function probing(): boolean {
  return active;
}

/** Attach or detach the seam. The fixture attaches before the page mounts; a release build never calls this. */
export function watchBoxFrames(on: boolean): void {
  active = on;
}

/** Report one frame, only while attached. */
export function reportBoxFrame(key: string, x: number, width: number): void {
  if (!active) return;
  const now = frames();
  const cur = now[key];
  if (cur && Math.abs(cur.x - x) < 0.5 && Math.abs(cur.width - width) < 0.5) return;
  frames.set({ ...now, [key]: { x, width } });
}

/** Register an input's async measure (native inputs take no onLayout); the fixture's probe interval polls them. */
export function registerMeasure(key: string, fn: () => Promise<{ pagePosition: { x: number }; bounds: { size: { width: number } } } | null>): void {
  if (!active) return;
  measurers.set(key, fn);
}

/** Re-measure every registered input (the fixture calls this on its probe interval). */
export function pollBoxFrames(): void {
  if (!active) return;
  for (const [k, fn] of measurers) {
    void fn()
      .then((m) => {
        if (m) reportBoxFrame(k, m.pagePosition.x, m.bounds.size.width);
      })
      .catch(() => {});
  }
}

/** The frames so far. */
export function boxFrames(): Record<string, BoxFrame> {
  return frames();
}
