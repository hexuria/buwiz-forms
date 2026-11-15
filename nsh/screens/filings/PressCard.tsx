// A card that is one press target: the whole surface opens its return. The kit has no pressable surface yet (ListRow is a
// full-width row, AppButton a 32-point control), so this is the one place Filings touches nsh/base. A button inside it (Cancel
// queue) takes its own press: only the innermost pressable view responds (NSH touch-and-gestures.md).
import type { JSX } from "nsh";
import { Button } from "nsh/base";

/** Props of PressCard. */
export interface PressCardProps {
  /** What screen readers say of the whole card. */
  label: string;
  testID?: string;
  onPress: () => void;
  children?: JSX.Element;
}

/** A bordered surface on the card fill (radius 8, 12 by 10 inside) that gains the strong border on hover and the keyboard ring on focus. */
export function PressCard(props: PressCardProps) {
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.label}
      onPress={props.onPress}
      class="w-full flex-col items-stretch gap-1 rounded-(--radius-row) border border-border bg-card px-3 py-2.5 enabled:hover:border-border-strong active:bg-selected focus-visible:ring-2 focus-visible:ring-accent-blue"
    >
      {props.children}
    </Button>
  );
}
