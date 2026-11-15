// What the frame may ask of the form page that is open (the old app's `app.formUi`): step to the next or previous to-do,
// which the palette offers as "Next to do" and "Previous to do" while a form shows. The page registers itself when it is
// made and leaves when it is disposed; nothing here imports the frame.
import { signal, untracked } from "nsh";

/** The handle a form page gives the frame. */
export interface FormUiHandle {
  /** The page shows its pill (the form is loaded and not in error): a step means something. */
  pillShown(): boolean;
  /** The pill's step: +1 next to-do, -1 previous (the page's own Cmd/Ctrl+Right and Left). */
  step(dir: number): void;
}

const open = signal<FormUiHandle | null>(null);

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
    return () => {
      if (untracked(open) === ui) open.set(null);
    };
  },
};
