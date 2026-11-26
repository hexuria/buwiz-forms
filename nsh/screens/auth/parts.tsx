// The pieces every gate and lock screen is made of (the old shell's auth.js screen, heading, labelled, errorLine, link,
// actions, choice). The kit has no text link and no pressable tile, so TextLink and ChoiceTile wrap one nsh/base Button
// here, as screens/filings/PressCard.tsx does; everything else is the kit.
import type { JSX } from "nsh";
import { Scroll, View } from "nsh";
import { Button } from "nsh/base";
import { deviceCopy, Icon, Text, TOUCH, type IconName } from "../../ui";
import { authScrollStyle } from "./signInLook";

export { deviceCopy };

/**
 * The card scroll content of every locked page: centred in the window with a pointer; on touch anchored near the top of what is
 * visible, so the keyboard comes up under it and a sentence appearing in the card never moves a field. Touch layout only.
 */
export const AUTH_SCROLL_CONTENT = TOUCH ? "grow items-center px-4 pt-6 pb-6" : "grow items-center justify-center px-4 py-6";

/**
 * The 400-wide card (the full width less 16 on each side when narrower) in a scroll view: centred in the window with a pointer,
 * near the top on touch. Being a scroll view, it is what NSH pads and scrolls when the keyboard covers its focused field
 * (nsh docs, Forms and the keyboard: "Inside scroll views: automatic"); a tap on a button while the keyboard is up presses it. On
 * touch it also ends at the keyboard's top (signInLook.ts authScrollStyle), so the whole card can be scrolled above it.
 */
export function AuthCard(props: { testID: string; shadow?: boolean; children?: JSX.Element }) {
  return (
    <Scroll
      testID="auth-scroll"
      keyboardShouldPersistTaps="handled"
      class="min-h-0 flex-1"
      style={authScrollStyle(TOUCH)}
      contentClass={AUTH_SCROLL_CONTENT}
    >
      <View
        testID={props.testID}
        class={`w-full gap-4 rounded-(--radius-dialog) border border-border bg-card p-6 wide:p-8 ${props.shadow === false ? "" : "shadow-lg"}`}
        style={{ maxWidth: 400 }}
      >
        {props.children}
      </View>
    </Scroll>
  );
}

/** The card's head: a 44-point round badge with the icon, the title (20/600) and a line (12 fg2), all centred. */
export function Heading(props: { icon: IconName; title: string; line: string; testID?: string }) {
  return (
    <View class="items-center gap-2">
      <View class="size-11 items-center justify-center rounded-full bg-hover">
        <Icon name={props.icon} tone="fg2" />
      </View>
      <Text variant="page" class="text-center" testID={props.testID}>{deviceCopy(props.title)}</Text>
      <Text variant="meta" tone="fg2" class="text-center">{deviceCopy(props.line)}</Text>
    </View>
  );
}

/** A label (12 fg2) over its control. */
export function Labelled(props: { label: string; children?: JSX.Element }) {
  return (
    <View class="gap-2">
      <Text variant="meta" tone="fg2">{props.label}</Text>
      {props.children}
    </View>
  );
}

/** The error under the boxes, in danger. Nothing when there is none. */
export function ErrorLine(props: { message: string | null; testID?: string }) {
  return (
    <If when={props.message}>
      <Text variant="meta" tone="danger" testID={props.testID ?? "auth-error"}>{deviceCopy(props.message as string)}</Text>
    </If>
  );
}

/** A link: 12/500 in the accent, a press target the size of its words (44 tall on touch). */
export function TextLink(props: { label: string; testID: string; onPress: () => void }) {
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.label}
      onPress={props.onPress}
      class={`self-start justify-center rounded-(--radius-ctl) enabled:hover:opacity-80 focus-visible:ring-2 focus-visible:ring-accent-blue ${TOUCH ? "min-h-11" : ""}`}
    >
      <Text variant="metaStrong" tone="accent">{props.label}</Text>
    </Button>
  );
}

/** Left and right ends of the card's last row: links or Back on the left, the main button on the right. */
export function Actions(props: { left?: JSX.Element; right: JSX.Element }) {
  return (
    <View class="flex-row items-center justify-between gap-2">
      <View class="min-w-0 shrink">{props.left}</View>
      {props.right}
    </View>
  );
}

/** One answer to "Who will use eBIRForms?": a bordered tile, the icon in the accent, a title and what it means. */
export function ChoiceTile(props: { testID: string; icon: IconName; title: string; line: string; onPress: () => void }) {
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.title}
      onPress={props.onPress}
      class="w-full flex-row items-start gap-3 rounded-(--radius-card) border border-border p-4 enabled:hover:bg-hover active:bg-selected focus-visible:ring-2 focus-visible:ring-accent-blue"
    >
      <Icon name={props.icon} size={20} tone="accent" />
      <View class="min-w-0 flex-1 gap-1">
        <Text variant="control">{props.title}</Text>
        <Text variant="meta" tone="fg2">{props.line}</Text>
      </View>
    </Button>
  );
}
