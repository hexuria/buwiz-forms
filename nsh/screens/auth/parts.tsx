// The pieces every page before the app is made of (the old shell's auth.js screen, heading, labelled, errorLine, link,
// actions, choice): the open page (no card: the Buwiz Forms logo on top, the page under it), its back control and its big
// button, and what the PIN sign-in, setup and locked pages put on it. The kit has no text link and no pressable tile, so
// TextLink, ChoiceTile, BackControl and PageButton wrap one nsh/base Button here, as screens/filings/PressCard.tsx does;
// everything else is the kit.
import type { JSX } from "nsh";
import { platform, safeArea, Scroll, Text as NText, View, windowSize } from "nsh";
import { Button } from "nsh/base";
import { deviceCopy, Icon, type IconName, Spinner, T_MIN_H, Text, TOUCH } from "../../ui";
import { BrandLogo } from "./brand";
import { authScrollStyle, backRowHeight, firstPageBottom, OPEN_SIDE, openColumnMax, openTop, signInHeights } from "./signInLook";

export { deviceCopy };

const ANDROID = platform.os === "android";

/**
 * The open page every page before the app is drawn on (the owner's look B): no card, a column (340 points wide with a pointer,
 * 400 on touch) with a row for the back control, the whole logo under it in the same place on every page, then the page.
 * `footer` is the first sign-in page's button: on touch it sits low (the column fills the screen), with a pointer 28 under the
 * page. A scroll view, so it is what NSH pads and scrolls when the keyboard covers its focused field (nsh docs, Forms and the
 * keyboard: "Inside scroll views: automatic"); a tap on a button while the keyboard is up presses it. On touch it also ends at
 * the keyboard's top (signInLook.ts authScrollStyle), so the whole page can be scrolled above it.
 */
export function OpenPage(props: { testID: string; back?: JSX.Element; footer?: JSX.Element; gap?: string; children?: JSX.Element }) {
  const low = TOUCH && props.footer !== undefined;
  return (
    <Scroll testID="auth-scroll" keyboardShouldPersistTaps="handled" class="min-h-0 flex-1" style={authScrollStyle(TOUCH)} contentClass="grow items-center">
      <View
        testID={props.testID}
        class={low ? "w-full flex-1 justify-between gap-7" : "w-full gap-7"}
        style={{
          maxWidth: openColumnMax(TOUCH) + 2 * OPEN_SIDE,
          paddingLeft: OPEN_SIDE,
          paddingRight: OPEN_SIDE,
          paddingTop: openTop(TOUCH, windowSize().height),
          paddingBottom: low ? firstPageBottom(safeArea().bottom) : 24,
        }}
      >
        <View class="w-full">
          <View class="w-full flex-row items-center" style={{ height: backRowHeight(TOUCH, ANDROID) }}>
            {props.back}
          </View>
          <View class={`w-full ${props.gap ?? "gap-4"}`} style={{ marginTop: 18 }}>
            <View class="mb-2">
              <BrandLogo />
            </View>
            {props.children}
          </View>
        </View>
        <If when={props.footer !== undefined}>
          <View class="w-full gap-2.5">{props.footer}</View>
        </If>
      </View>
    </Scroll>
  );
}

/**
 * A page before the app on the open page: the PIN sign-in (and Forgot PIN), each first-run step, the locked page. The id names
 * the page's column, as it named the card these pages were drawn in before.
 */
export function AuthCard(props: { testID: string; children?: JSX.Element }) {
  return (
    <OpenPage testID={props.testID} gap="gap-4">
      {props.children}
    </OpenPage>
  );
}

/** The back control at the top left: a chevron and its words in the accent, 40 points tall (a whole touch target on touch). */
export function BackControl(props: { testID: string; label: string; onPress: () => void }) {
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.label}
      onPress={props.onPress}
      class="flex-row items-center gap-1 rounded-(--radius-ctl) pr-2 enabled:hover:opacity-80 focus-visible:ring-2 focus-visible:ring-accent-blue"
      style={{ height: backRowHeight(TOUCH, ANDROID) }}
    >
      <Icon name="chevron-left" size={18} tone="accent" />
      <NText class="font-medium text-accent-blue" style={{ fontSize: 15 }}>{props.label}</NText>
    </Button>
  );
}

/**
 * The sign-in pages' main button: the full width, 46 points tall (48 on Android), radius 10, ink with the page's colour on it,
 * 15/600; its spinner (said as the label and "…") while its own request runs.
 */
export function PageButton(props: { testID: string; label: string; icon?: IconName; busy?: boolean; onPress: () => void }) {
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.label}
      onPress={props.onPress}
      class="w-full flex-row items-center justify-center gap-2 rounded-[10px] bg-primary px-4 enabled:hover:opacity-90 active:opacity-80 disabled:opacity-50 focus-visible:ring-2 focus-visible:ring-accent-blue"
      style={{ minHeight: signInHeights(ANDROID).button }}
    >
      <If when={props.busy} else={props.icon ? <Icon name={props.icon} tone="primaryFg" /> : null}>
        <Spinner size={14} tone="primaryFg" label={`${props.label}…`} />
      </If>
      <NText class="font-semibold text-primary-foreground" style={{ fontSize: 15 }}>{props.label}</NText>
    </Button>
  );
}

/** The sign-in pages' title: 22-point bold, centred. */
export function PageHeading(props: { testID?: string; children?: JSX.Element }) {
  return (
    <NText testID={props.testID} class="text-center font-bold text-foreground" style={{ fontSize: 22 }}>
      {props.children}
    </NText>
  );
}

/** A soft sentence of the sign-in pages, centred: 14 points by default. */
export function SoftLine(props: { testID?: string; size?: number; children?: JSX.Element }) {
  return (
    <NText testID={props.testID} class="text-center text-muted-foreground" style={{ fontSize: props.size ?? 14 }}>
      {props.children}
    </NText>
  );
}

/** The page's head under the logo: the title (20/600) and a line (12 fg2), centred. */
export function Heading(props: { title: string; line: string; testID?: string }) {
  return (
    <View class="items-center gap-2">
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

/** A link: 12/500 in the accent, centred, a press target the size of its words (44 tall on touch). */
export function TextLink(props: { label: string; testID: string; onPress: () => void }) {
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.label}
      onPress={props.onPress}
      class={`self-center justify-center rounded-(--radius-ctl) enabled:hover:opacity-80 focus-visible:ring-2 focus-visible:ring-accent-blue ${TOUCH ? T_MIN_H : ""}`}
    >
      <Text variant="metaStrong" tone="accent">{props.label}</Text>
    </Button>
  );
}

/** The page's last part: the main button (or buttons) at the full width, then the links or Back centred under it. */
export function Actions(props: { left?: JSX.Element; right: JSX.Element }) {
  return (
    <View class="w-full gap-2">
      <View class="w-full items-stretch gap-2">{props.right}</View>
      <If when={props.left !== undefined}>
        <View class="w-full items-center">{props.left}</View>
      </If>
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
