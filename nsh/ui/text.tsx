// Text and marks: the type scale as one Text, one-line Line, Code (TINs, form codes), titles and labels, MetaLine,
// Icon, Kbd, Hairline, Spinner. Every color is a theme token class; sizes come from theme/tokens.ts TYPE.
import type { JSX } from "nsh";
import { Image, Text as NText, View } from "nsh";
import { TYPE } from "../theme/tokens";
import { type IconName, ICONS } from "./icons";
import { cx, LEADING, TEXT_TONE, TINT_TONE, type Tone } from "./look";

/** A role of the type scale: page 20, title 16, emph 14, body 13, control 13/500, meta 12, metaStrong 12/500, cap 11, num 12/600 tabular. */
export type TypeRole = keyof typeof TYPE;

/** Props of Text. */
export interface TextProps {
  /** The type role (default body). */
  variant?: TypeRole;
  /** The color (default fg). */
  tone?: Tone;
  /** Layout and extra classes (margins, `text-right`, `tabular-nums`, `flex-1`). */
  class?: string;
  /** At most this many lines, the last one ending in an ellipsis. */
  lines?: number;
  testID?: string;
  children?: JSX.Element;
}

/** A run of text in one role of the type scale and one tone. */
export function Text(props: TextProps) {
  return (
    <NText
      testID={props.testID}
      class={cx(TYPE[props.variant ?? "body"].cls, LEADING, TEXT_TONE[props.tone ?? "fg"], props.class)}
      style={{ numberOfLines: props.lines }}
    >
      {props.children}
    </NText>
  );
}

/** One line that truncates with an ellipsis inside its box (it may shrink in a row). */
export function Line(props: Omit<TextProps, "lines">) {
  return <Text variant={props.variant} tone={props.tone} testID={props.testID} lines={1} class={cx("min-w-0 shrink", props.class)}>{props.children}</Text>;
}

/** TINs, branch codes and form codes: 12/600 in tabular figures, never wrapping. */
export function Code(props: { children?: JSX.Element; tone?: Tone; class?: string; testID?: string }) {
  return <Text variant="num" tone={props.tone} testID={props.testID} lines={1} class={props.class}>{props.children}</Text>;
}

/** The page title, one per view: 20/600. */
export function PageTitle(props: { children?: JSX.Element; class?: string }) {
  return <Text variant="page" class={props.class}>{props.children}</Text>;
}

/** A section heading: 16/600, 32 above it (none for the `first`), 12 below. */
export function SectionTitle(props: { children?: JSX.Element; first?: boolean }) {
  return <Text variant="title" class={props.first ? "mb-3" : "mt-8 mb-3"}>{props.children}</Text>;
}

/** A group label in sidebars and panels: 12/500 in fg3, 16 above, 4 below, 8 in. */
export function GroupLabel(props: { children?: JSX.Element }) {
  return (
    <View class="px-2 pt-4 pb-1">
      <Text variant="metaStrong" tone="fg3">{props.children}</Text>
    </View>
  );
}

/** Meta items on one wrapping line, 3-point dots between them; a string is 12 in fg2. */
export function MetaLine(props: { parts: (string | JSX.Element | null | undefined | false)[] }) {
  const parts = () => props.parts.filter((p) => p !== null && p !== undefined && p !== false && p !== "");
  return (
    <View class="flex-row flex-wrap items-center gap-2">
      <Each of={parts()}>
        {(p, i) => (
          <View class="flex-row items-center gap-2">
            <If when={i() > 0}>
              <View class="size-0.75 rounded-full bg-subtle-foreground" />
            </If>
            {typeof p() === "string" ? <Text variant="meta" tone="fg2">{p() as string}</Text> : (p() as JSX.Element)}
          </View>
        )}
      </Each>
    </View>
  );
}

/** Props of Icon. */
export interface IconProps {
  name: IconName;
  /** Points: 16 everywhere, 12 in chips, 14 in pills and segments (default 16). */
  size?: number;
  /** The tint (default fg2). */
  tone?: Tone;
  /** Extra classes (state variants of the tint: `group-selected:tint-foreground`, `animate-spin`). */
  class?: string;
}

/** A Lucide icon from assets/icons, tinted with a theme token. */
export function Icon(props: IconProps) {
  return (
    <Image
      src={ICONS[props.name]}
      class={cx("shrink-0", TINT_TONE[props.tone ?? "fg2"], props.class)}
      style={{ width: props.size ?? 16, height: props.size ?? 16 }}
    />
  );
}

/** A keyboard hint: 11/500 fg3 in a hairline box, 18 tall. */
export function Kbd(props: { children?: JSX.Element }) {
  return (
    <View class="h-4.5 shrink-0 flex-row items-center rounded-sm border border-border px-1">
      <Text variant="cap" tone="fg3">{props.children}</Text>
    </View>
  );
}

/** A hairline: between list rows and disclosure sections only. */
export function Hairline() {
  return <View class="h-px w-full shrink-0 bg-border" />;
}

/** A spinning loader mark (the loader icon), for work in progress; `label` is what screen readers say. */
export function Spinner(props: { size?: number; tone?: Tone; label?: string }) {
  return (
    <View accessibilityLabel={props.label ?? "Loading"} class="shrink-0 items-center justify-center">
      <Icon name="loader" size={props.size} tone={props.tone ?? "fg3"} class="animate-spin" />
    </View>
  );
}
