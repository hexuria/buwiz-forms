// What the frame may ask of the form page that is open (the old app's `app.formUi`): step to the next or previous to-do,
// which the palette offers as "Next to do" and "Previous to do" while a form shows; and what an agent's commands ask
// (shell/agent): wait for the return to be open, commit its fill, mark it ready for review. The page registers itself when
// it is made and leaves when it is disposed; nothing here imports the frame.
import { signal, untracked } from "nsh";

/** Which return a page shows. */
export interface FormUiTarget {
  tin: string;
  formCode: string;
  year: number;
  period: number;
}

/** The handle a form page gives the frame. */
export interface FormUiHandle {
  /** The page shows its pill (the form is loaded and not in error): a step means something. */
  pillShown(): boolean;
  /** The pill's step: +1 next to-do, -1 previous (the page's own Cmd/Ctrl+Right and Left). */
  step(dir: number): void;
  /** The return the page shows: an agent's fill waits for the one it opened. */
  target?(): FormUiTarget;
  /** Resolves once the page finished opening the return (or failed to). */
  ready?(): Promise<void>;
  /** Commit these edits now, as a choice: what a pick commits (the agent's fill, applied as the old app's `choose`). */
  choose?(edits: Record<string, string>, clear?: string[]): void;
}

const open = signal<FormUiHandle | null>(null);
/** The agent's message while the open return is prepared for the person's review (the Queue button leads). */
const review = signal<string | null>(null);

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

export const formUi = {
  /** A form page is open and shows its pill. Reactive. */
  canStep: (): boolean => open()?.pillShown() ?? false,
  /** Step to the next (+1) or previous (-1) to-do of the open form; nothing when none is open. */
  step: (dir: number): void => {
    const ui = open();
    if (ui?.pillShown()) ui.step(dir);
  },
  /** Called by the page: it is the open form until the returned function runs (a page made later stays the open one). */
  attach(ui: FormUiHandle): () => void {
    open.set(ui);
    review.set(null);
    return () => {
      if (untracked(open) === ui) {
        open.set(null);
        review.set(null);
      }
    };
  },
  /** The agent prepared the open return: its message, while the page should say so and lead with Queue. Reactive. */
  reviewing: (): string | null => review(),
  /** Marks the open return as prepared by the agent (it ends with the page). */
  review(message: string): void {
    review.set(message);
  },
  /**
   * Waits until the page of this return is open and loaded: the agent's fill and review land there, never on the page
   * of an earlier return that the new one has not replaced yet. False when it did not come within `timeoutMs`.
   */
  async whenReady(want: FormUiTarget, timeoutMs = 10_000): Promise<boolean> {
    const until = Date.now() + timeoutMs;
    while (Date.now() < until) {
      const ui = untracked(open);
      const at = ui?.target?.();
      if (ui && at && at.tin === want.tin && at.formCode === want.formCode && at.year === want.year && at.period === want.period) {
        await ui.ready?.();
        return untracked(open) === ui;
      }
      await sleep(100);
    }
    return false;
  },
  /** Commits `edits` on the open page as a choice; false when no page that can is open. */
  choose(edits: Record<string, string>, clear?: string[]): boolean {
    const ui = untracked(open);
    if (!ui?.choose) return false;
    ui.choose(edits, clear);
    return true;
  },
};
