// The pieces every page before the app is made of (the old shell's auth.js screen, heading, labelled, errorLine, link,
// actions, choice): the open page (no card: a back row, and the Buwiz Forms logo with the page under it), its back control and its big
// button, and what the PIN sign-in, setup and locked pages put on it. The kit has no text link and no pressable tile, so
// TextLink, ChoiceTile, BackControl and PageButton wrap one nsh/base Button here, as screens/filings/PressCard.tsx does;
// everything else is the kit.
import type { JSX, ScrollRef } from "nsh";
import { effect, keyboardFrame, platform, safeArea, Scroll, signal, Text as NText, untracked, View, windowSize } from "nsh";
import { Button } from "nsh/base";
import { BREAK, SIZE } from "../../theme/tokens";
import { deviceCopy, Icon, type IconName, Spinner, T_MIN_H, Text, TOUCH } from "../../ui";
import { BrandLogo } from "./brand";
import {
  blockRoom, backRow, bottomPad, keyboardCover, LOGO_GAP, logoStep, logoSteps, OPEN_SIDE, openColumnMax, openSpacing, signInHeights,
  underBackRowGuess,
} from "./signInLook";

export { deviceCopy };

const ANDROID = platform.os === "android";

/** What the open page now showing measured and chose (a fixture's readout prints it; nothing in the app reads it). */
export interface OpenFit {
  page: string;
  room: number;
  rest: number;
  logo: number;
  fits: boolean;
}
const openFit = signal<OpenFit | null>(null);
/** The open page's last fit (AuthDemo.tsx's readout). */
export function lastOpenFit(): OpenFit | null {
  return openFit();
}

/** Whether the open page now showing draws tight (signInLook.ts openSpacing): its pieces' gaps step down with it. */
const tightNow = signal(false);
export function openTight(): boolean {
  return tightNow();
}

/** The height of what sits over the open page in the locked screen (the welcome note), which the page's top is under. */
const abovePage = signal(0);
export function setAbovePage(height: number): void {
  abovePage.set(height);
}

/** A gap class one step smaller when the page draws tight. */
function tighter(gap: string, tight: boolean): string {
  if (!tight) return gap;
  return { "gap-4": "gap-3", "gap-3": "gap-2" }[gap] ?? gap;
}

/**
 * The open page every page before the app is drawn on (the owner's layout C, look B inside it): a fixed back row at the top,
 * under the window's title bar and inside the safe area, the same on every page (empty where there is no back control); under
 * it one block, the whole logo, then the page, then `footer` (the first sign-in page's button, close under its line), centred
 * in the room left. The logo takes the largest of its steps (signInLook.ts logoStep) at which the whole block fits that room,
 * measured as it lays out; only when even the smallest does not fit does the block scroll. The column is 340 points wide with
 * a pointer, 400 on touch (`wide`: the frame's page column, for "Just me"'s New taxpayer page).
 *
 * The block is in a scroll view, so it is what NSH pads and scrolls when the keyboard covers its focused field (nsh docs, Forms
 * and the keyboard: "Inside scroll views: automatic"); a tap on a button while the keyboard is up presses it. While the software
 * keyboard is up the room is what shows above it (signInLook.ts keyboardCover: the keyboard less what the platform already took
 * off the page, so an Android window laid out above the keyboard is not lifted twice), and the block centres above the keyboard
 * (its bottom padding): the logo steps down first, and only then does the block scroll, under the back row, never into it.
 */
export function OpenPage(props: { testID: string; back?: JSX.Element; footer?: JSX.Element; gap?: string; wide?: boolean; children?: JSX.Element }) {
  const row = () => backRow(TOUCH, ANDROID, safeArea().left);
  // The height under the back row (measured; before that, the window's) and the block's height under the logo (measured;
  // before that, a guess of a page's height, so the first frame mostly draws the step it keeps).
  const under = signal(0);
  const rest = signal(LOGO_GAP + 6 * signInHeights(ANDROID).button);
  // What the window gives under the back row, from this page's own top (under the welcome note when one shows over it).
  const guess = () => underBackRowGuess(windowSize().height, safeArea().top, TOUCH, ANDROID) - abovePage();
  const measured = () => (under() > 0 ? under() : guess());
  // The keyboard's part the page still has to end above (0 while it is down, and on a pointer).
  const cover = () => keyboardCover(keyboardFrame().height, guess(), measured());
  const spacing = () => openSpacing(blockRoom(measured(), cover(), safeArea().bottom));
  const room = () => blockRoom(measured(), cover(), safeArea().bottom, spacing().pad);
  const fit = () => logoStep(logoSteps(TOUCH, windowSize()), room(), rest());
  effect(() => {
    const now = fit();
    tightNow.set(spacing().tight);
    openFit.set({ page: props.testID, room: room(), rest: rest(), logo: now.logo, fits: now.fits });
  });
  // Under the block: its padding and the bottom safe area ("Just me"'s long page keeps the frame's 56 under its end), and the
  // keyboard's part, so the block centres in what shows above the keyboard. The scroll view itself keeps its frame: the kit
  // gives it a bottom inset for the keyboard and scrolls to the focused field when the keyboard comes, from the layout before
  // the app answers; a scroll view the app shrank as well kept that inset and that scroll (the iOS proof of 1cf908a6: an empty
  // band over the keyboard, the logo and heading scrolled away).
  const padBottom = () => Math.max(props.wide ? 56 : 0, bottomPad(spacing().pad, cover(), safeArea().bottom)) + cover();
  // When the block fits, nothing should be scrolled: after the keyboard moves (the kit scrolled to the field from the layout
  // before the logo stepped down), and whenever the fit changes, the scroll goes back to its top, once at once and once the
  // keyboard's own motion is over. A block that does not fit keeps the kit's scroll to the focused field.
  let scroller: ScrollRef | undefined;
  effect(() => {
    const keyboard = keyboardFrame();
    const fits = fit().fits;
    untracked(() => {
      if (!fits) return;
      const top = () => scroller?.scrollTo({ y: 0, animated: false });
      setTimeout(top, 0);
      if (keyboard.duration > 0) setTimeout(top, keyboard.duration + 50);
    });
  });
  const side = () => (props.wide ? (windowSize().width < BREAK.narrowContent ? 16 : 32) : OPEN_SIDE);
  const column = () => (props.wide ? SIZE.contentMax : openColumnMax(TOUCH)) + 2 * side();
  return (
    <View testID="open-page" class="min-h-0 flex-1">
      <View testID="open-back-row" class="w-full shrink-0 flex-row items-center" style={{ height: row().height, paddingLeft: row().left, paddingRight: safeArea().right }}>
        {props.back}
      </View>
      {/* The height under the back row as the page lays out: the platform's own shrinking for the keyboard (Android) shows here. */}
      <View class="min-h-0 flex-1" onLayout={(e) => e.bounds.size.height > 0 && under.set(Math.round(e.bounds.size.height))}>
        <Scroll
          testID="auth-scroll"
          ref={(r: ScrollRef) => (scroller = r)}
          keyboardShouldPersistTaps="handled"
          class="min-h-0 flex-1"
          contentClass="grow items-center justify-center"
        >
          <View
            testID={props.testID}
            class="w-full"
            style={{ maxWidth: column(), paddingLeft: side(), paddingRight: side(), paddingTop: spacing().pad, paddingBottom: padBottom() }}
          >
            <BrandLogo height={fit().logo} />
            <View class="w-full" style={{ paddingTop: spacing().logoGap }} onLayout={(e) => e.bounds.size.height > 0 && rest.set(Math.round(e.bounds.size.height))}>
              <View class={`w-full ${tighter(props.gap ?? "gap-4", spacing().tight)}`}>{props.children}</View>
              <If when={props.footer !== undefined}>
                <View class={`${spacing().tight ? "mt-4" : "mt-6"} w-full gap-2.5`}>{props.footer}</View>
              </If>
            </View>
          </View>
        </Scroll>
      </View>
    </View>
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

/**
 * The back control, in the open page's back row (top left, under the title bar, inside the safe area): a chevron and its words
 * in the accent, as tall as the row (40 points; a whole touch target on touch).
 */
export function BackControl(props: { testID: string; label: string; onPress: () => void }) {
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.label}
      onPress={props.onPress}
      class="flex-row items-center gap-1 rounded-(--radius-ctl) pr-2 enabled:hover:opacity-80 focus-visible:ring-2 focus-visible:ring-accent-blue"
      style={{ height: backRow(TOUCH, ANDROID, 0).height }}
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
    <View class={openTight() ? "items-center gap-1" : "items-center gap-2"}>
      <Text variant="page" class="text-center" testID={props.testID}>{deviceCopy(props.title)}</Text>
      <Text variant="meta" tone="fg2" class="text-center">{deviceCopy(props.line)}</Text>
    </View>
  );
}

/** A label (12 fg2) over its control. */
export function Labelled(props: { label: string; children?: JSX.Element }) {
  return (
    <View class={openTight() ? "gap-1" : "gap-2"}>
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
