// The two layouts of a labelled box. Wide: the label on the left, the control on the right (as the old page). Narrow: the label
// above and the control across the whole width. The form page draws its own item labels; this is for places that label a single
// box themselves (a schedule row's drawer, the gallery).
import type { Accessor, JSX } from "nsh";
import { View } from "nsh";
import { Text } from "../../../ui";
import { cx } from "./model";

/** Width of the label column of the wide layout. */
export const LABEL_WIDTH = 220;

/** Props of FieldRow. */
export interface FieldRowProps {
  label: string;
  /** A line under the label (what the box is for). */
  hint?: string;
  narrow: Accessor<boolean>;
  testID?: string;
  children?: JSX.Element;
}

/** A label and its control, side by side when wide and stacked when narrow. */
export function FieldRow(props: FieldRowProps) {
  return (
    <View testID={props.testID} class={cx("w-full min-w-0", props.narrow() ? "gap-1" : "flex-row items-start gap-4")}>
      <View class={cx("gap-0.5", !props.narrow() && "pt-1.5")} style={{ width: props.narrow() ? undefined : LABEL_WIDTH }}>
        <Text variant="control">{props.label}</Text>
        <If when={props.hint}>
          <Text variant="meta" tone="fg3">{props.hint}</Text>
        </If>
      </View>
      <View class="min-w-0 flex-1">{props.children}</View>
    </View>
  );
}
