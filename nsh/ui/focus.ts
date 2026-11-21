import type { ViewRef } from "nsh";

/** Brings an editor into its enclosing scroll views before giving it keyboard focus. */
export function revealAndFocus(ref: Pick<ViewRef, "reveal" | "focus">, editor: Pick<ViewRef, "reveal"> = ref): void {
  editor.reveal({ vertical: "nearest", animated: false });
  ref.focus();
}
