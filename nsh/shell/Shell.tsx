// The frame around every screen (old app/main.js paint): the app root. It boots the app state, owns the polls and the
// key bindings (both end with it), draws the title bar, sidebar, main surface and right panel (wide), or the header, the
// tab bar and the sheets (under 600 points), floats the sidebar and panel as overlays under 860, and mounts the dialogs
// and the toaster once. Screens are drawn by views.tsx.
import type { JSX } from "nsh";
import { cleanup, computed, effect, ErrorBoundary, keymap, platform, safeArea, Scroll, untracked, View } from "nsh";
import { Button } from "nsh/base";
import { isAvailable as desktopAvailable, currentWindow, focusedWindow, titleBar } from "nsh/desktop";
import { BackHandler, isAvailable as mobileAvailable } from "nsh/mobile";
import { refreshTins } from "../screens/taxpayer/tins";
import { errorText } from "../lib/errors";
import { LockedScreen } from "../screens/auth/Locked";
import { SIZE } from "../theme/tokens";
import { AppButton, Text, Toaster, TOUCH } from "../ui";
import { app, boot, startPolling, startQueuedWatch, warmLibrary } from "./app";
import { DevPages } from "./DevPages";
import { startDevCommandMailbox } from "./devCommand";
import { NarrowBar, PanelSheets, TabBar } from "./Narrow";
import { NewReturn } from "./NewReturn";
import { Palette } from "./Palette";
import { RightPanel } from "./Panels";
import { contentBox, ROUTES, type Route } from "./routes";
import { startAgentBridge } from "./agent";
import { Sidebar } from "./Sidebar";
import { store } from "./store";
import { TitleBar } from "./TitleBar";
import { chromeHeight, chromeNow } from "./chrome";
import { toastPlace } from "./chrome_layout";
import { bindMenuKeys, MenuListButton, MenuRow } from "./MenuRow";
import { UpdateSlot } from "./UpdateSlot";
import { renderView } from "./views";

/** Props of Shell. */
export interface ShellProps {
  /** The dev start route (BIR_SHELL_START): "tp=<root>;theme=dark;view=library;form=2551Q/2026/3". */
  start?: string;
  /** Run the polls (events 30 s, device filings 5 s, update status 1 s). Default true; test fixtures turn it off. */
  polling?: boolean;
  /** While any return is Queued, read the lists every 3 s, for about 30 s after the Queued set last changed (the worker sends it within seconds and tells nobody). Default: with the polls; the app turns it on in the dev environment too. */
  watchQueued?: boolean;
  /** Build the form library's index a moment after the frame is first shown (not behind the sign-in), so the first Form library visit is not the one that waits. The app turns it on; fixtures leave it off. */
  warm?: boolean;
  /** First-run setup in front of the frame while the database has no access mode (the shipped app; fixtures leave it off). */
  firstRun?: boolean;
  /** Start the agent remote control and apply its commands (the shipped app, also in the dev environment; fixtures leave it off). Inert unless the Rust feature `agent` is built in and GPUI_AGENT=1. */
  agent?: boolean;
}

/** The chords the frame owns (Cmd on macOS, Ctrl elsewhere), and Esc. Each has one owner: this root. */
function bindKeys(): void {
  // The gate, first launch and sign-in have none of the frame's commands (the old app-root that handled them was not drawn).
  const open = (run: () => unknown) => () => void (app.chromeVisible() && run());
  keymap.bind(
    {
      "mod+b": open(() => app.toggleSidebar()),
      "mod+t": open(() => app.cycleTheme()),
      "mod+shift+b": open(() => app.togglePanel("profile")),
      "mod+shift+n": open(() => app.togglePanel("notifs")),
      "mod+k": open(() => app.openPalette()),
      "mod+n": open(() => app.startNewReturn()),
      "mod+,": open(() => app.go("settings")),
      "mod+l": () => app.lockApp(),
      "mod+[": open(() => app.back()),
      "mod+]": open(() => app.forward()),
    },
    { when: "always" },
  );
  keymap.bind("escape", () => void app.dismiss(), { when: "always" });
}

/** A screen that threw: the frame stays, the page says so and offers Home. */
function ViewError(props: { error: unknown; reset: () => void }) {
  store.logScreen("error", `screen: ${errorText(props.error)}`);
  return (
    <View testID="view-error" class="items-start gap-3 p-8">
      <Text variant="title">This page could not be shown</Text>
      <Text tone="fg2">{errorText(props.error)}</Text>
      <AppButton
        testID="view-error-home"
        label="Go to Home"
        variant="primary"
        onPress={() => {
          props.reset();
          app.go("home");
        }}
      />
    </View>
  );
}

function Guarded(props: { route: Route }): JSX.Element {
  return (
    <ErrorBoundary fallback={(error, reset) => <ViewError error={error} reset={reset} />}>
      {renderView(props.route)}
    </ErrorBoundary>
  );
}

/** A view in the main surface, laid out as its route table entry says (page 880, wide 1120, full, or self: the screen sets its own width and scrolls). */
function ViewFrame() {
  const route = untracked(app.route);
  const view = untracked(app.view);
  const layout = ROUTES[view].layout;
  if (layout === "self") {
    return (
      <View class="min-h-0 w-full flex-1">
        <Guarded route={{ ...route, view }} />
      </View>
    );
  }
  // The old rule: padding 32 (16 under 640 points of surface), the cap of 880 (1120, none) includes it (routes.ts contentBox).
  const box = () => contentBox(layout, app.layout.mainWidth());
  return (
    <Scroll
      testID="main-scroll"
      keyboardShouldPersistTaps="handled"
      class="min-h-0 flex-1"
      contentClass={`items-center pb-14 ${box().pad === 16 ? "px-4 pt-5" : "px-8 pt-7"}`}
    >
      <View testID="content-column" class="w-full min-w-0" style={{ maxWidth: box().maxWidth }}>
        <Guarded route={{ ...route, view }} />
      </View>
    </Scroll>
  );
}

/** The surface the screens draw on: inset 8 with radius 12 and a hairline when wide, flush when narrow. */
function MainSurface() {
  return (
    <View
      testID="main-surface"
      class={`min-h-0 min-w-0 flex-1 overflow-hidden border-border bg-card ${app.layout.narrow() ? "border-y" : "rounded-(--radius-card) border"}`}
      style={{
        marginBottom: app.layout.narrow() ? 0 : SIZE.mainInset,
        marginLeft: app.layout.narrow() || app.layout.sidebarDocked() ? 0 : SIZE.mainInset,
        marginRight: app.layout.narrow() || app.layout.panelDocked() ? 0 : SIZE.mainInset,
      }}
    >
      <View class="min-h-0 flex-1">
        {() => {
          // The view is made again when the person moves (the route's seq) or the view that shows changes; a retarget keeps it.
          void app.route().seq;
          void app.view();
          return untracked(() => <ViewFrame />);
        }}
      </View>
    </View>
  );
}

/** Under 860 points the sidebar and the panel float over the page, on a scrim that closes them. */
function Floating() {
  return (
    <If when={app.layout.overlayMode() && !app.layout.narrow() && (app.sidebarOpen() || app.panel() !== null)}>
      <Scrim />
      <If when={app.sidebarOpen()}>
        <View testID="sidebar-overlay" class="absolute bottom-0 left-0 bg-background shadow-xl" style={{ top: chromeHeight() }}>
          <Sidebar />
        </View>
      </If>
      <If when={app.panel() !== null}>
        <View testID="panel-overlay" class="absolute right-0 bottom-0 bg-background shadow-xl" style={{ top: chromeHeight() }}>
          <RightPanel />
        </View>
      </If>
    </If>
  );
}

/** A press on the scrim closes the sidebar and the panel; the layers above it never reach it. */
function Scrim() {
  return (
    <Button
      testID="overlay-scrim"
      accessibilityLabel="Close"
      class="absolute inset-x-0 bottom-0 bg-scrim"
      style={{ top: chromeHeight() }}
      onPress={app.closeFloating}
    />
  );
}

/** Wide and tablet widths: the toolbar, under the menu row on Windows and Linux without a Global Menu (chrome_layout.ts). */
function WideChrome() {
  return (
    <>
      <If when={chromeNow().menuRow}>
        <MenuRow />
      </If>
      <TitleBar />
    </>
  );
}

/** The signed-in frame. */
function Frame() {
  return (
    <View testID="frame" class="min-h-0 flex-1">
      <If when={app.layout.narrow()} else={<WideChrome />}>
        <NarrowBar />
      </If>
      {/* Keep off a phone's sensor housing turned sideways, and (wide) off the home indicator; narrow, the tab bar keeps off the bottom. */}
      <View
        testID="frame-body"
        class="min-h-0 flex-1 flex-row"
        style={
          app.layout.narrow()
            ? { paddingLeft: safeArea().left, paddingRight: safeArea().right }
            : { paddingLeft: safeArea().left, paddingRight: safeArea().right, paddingBottom: safeArea().bottom }
        }
      >
        <If when={app.layout.sidebarDocked() && !app.layout.narrow()}>
          <Sidebar />
        </If>
        <MainSurface />
        <If when={app.layout.panelDocked() && !app.layout.narrow()}>
          <RightPanel />
        </If>
      </View>
      <If when={app.layout.narrow()}>
        <TabBar />
        <PanelSheets />
      </If>
      <Floating />
      <If when={app.overlay() === "palette"}>
        <Palette />
      </If>
      <If when={app.overlay() === "new"}>
        <NewReturn />
      </If>
      <UpdateSlot />
    </View>
  );
}

/**
 * First launch, sign-in and the account gate take the whole window (old renderLockedFrame): only a drag strip with room for
 * the window controls; nothing that names a taxpayer, no theme, search or account. The update prompt shows over first
 * launch and sign-in, as before.
 */
function LockedFrame() {
  return (
    <View testID="locked-frame" class="min-h-0 flex-1">
      {/* The locked menu (About, Quit, editing) stays reachable where the window draws the menu: the menu row, or ☰ when narrow. */}
      {/* On a phone or tablet the strip is only the status bar's room: the page's back row is where a navigation bar would be. */}
      <If
        when={chromeNow().menuRow}
        else={
          <View
            testID="title-bar"
            desktop={{ windowDrag: true }}
            class="shrink-0 flex-row items-center"
            style={{ height: (TOUCH ? 0 : SIZE.titleBar) + safeArea().top, paddingTop: safeArea().top, paddingLeft: 8 + titleBar().left, paddingRight: titleBar().right }}
          >
            <If when={chromeNow().hamburger}>
              <MenuListButton testID="locked-menu" />
            </If>
          </View>
        }
      >
        <MenuRow />
      </If>
      <LockedScreen />
      <UpdateSlot locked />
    </View>
  );
}

/** The dev pages (the theme page and the UI kit's gallery) take the whole window. */
const isDevPage = () => app.view() === "kit" || app.view() === "theme";

/** The app root. */
const toastSpot = () => toastPlace(app.view(), app.layout.narrow(), TOUCH);

export function Shell(props: ShellProps) {
  void boot(props.start, { firstRun: props.firstRun === true });
  // D2: the account's TINs are asked at start and when the app becomes active (never on a timer).
  void refreshTins(0);
  let tinWasFocused = untracked(() => focusedWindow() !== null);
  let lastFocusRead = 0;
  effect(() => {
    const focused = focusedWindow() !== null;
    // A focus flicker is not a return from the account site: at most one read a minute, or the cache answers.
    if (focused && !tinWasFocused) {
      const now = Date.now();
      if (now - lastFocusRead >= 60_000) {
        lastFocusRead = now;
        void refreshTins(0);
      } else void refreshTins(60);
    }
    tinWasFocused = focused;
  });
  // The app draws its own 40-point title bar where the platform has windows (the traffic lights and caption buttons stay).
  // Two rows (Windows, Linux without a Global Menu): the title bar is the 32-point menu row, whose end holds the window's buttons.
  if (desktopAvailable && platform.desktop) {
    // A memo: the title bar is set again only when its height changes (two rows and one), not at every resize.
    const titleBarHeight = computed(() => chromeNow().titleBarHeight);
    effect(() => {
      const height = titleBarHeight();
      untracked(() => currentWindow().set({ titleBar: { style: "hidden", height } }));
    });
  }
  bindKeys();
  bindMenuKeys();
  // Android's back button: the topmost layer closes first, then history goes back; else the app leaves. While the frame is
  // hidden (the account gate, sign-in, first-run setup) the history behind it is not the person's to walk: the press is
  // left alone, so it leaves the app (the gate's own listener steps back on its email and code pages first).
  if (mobileAvailable && platform.os === "android") BackHandler.on("press", () => app.chromeVisible() && (app.dismiss() || app.back()));
  // The polls end with the root; headless fixtures leave them off unless a test asks.
  if (props.polling !== false) cleanup(startPolling());
  // A queued return is watched where the polls run, and in the dev environment too (the app asks); fixtures leave it off.
  if (props.watchQueued ?? props.polling !== false) cleanup(startQueuedWatch());
  if (props.warm === true) cleanup(warmLibrary());
  // The re-reads a queue action leaves pending (1 s, 3 s, 10 s later) end with the root.
  cleanup(() => store.cancelRereads());
  // The agent remote control (Rust feature `agent`, GPUI_AGENT=1): its commands are applied here, polled only while it listens.
  // Not tied to `polling`: the dev environment runs with the polls off, and an agent run is a dev run.
  if (props.agent === true) cleanup(startAgentBridge());
  cleanup(startDevCommandMailbox());
  // Under 860 points the sidebar and the panel float and start closed; back above it the sidebar docks open again.
  effect(() => {
    const overlays = app.layout.overlayMode();
    untracked(() => app.followWidth(overlays));
  });
  // The menu bar matches the window: the filing chrome shows only once nothing stands in front of it.
  effect(() => app.syncShellChrome(app.chromeVisible()));
  // The agent's host refuses while the app is locked or a taxpayer is signed in (src/agent.rs): it hears of every change.
  effect(() => app.syncAgentAccess());
  return (
    <View testID="shell" class="flex-1 bg-background">
      <If when={app.booted()}>
        <If when={app.chromeVisible()} else={<LockedFrame />}>
          <If when={isDevPage()} else={<Frame />}>
            <DevPages />
          </If>
        </If>
      </If>
      {/* The kit's gallery brings a Toaster of its own: two would show every toast twice. */}
      <If when={!isDevPage()}>
        <Toaster
          top={toastSpot().edge === "top" ? toastSpot().offset : undefined}
          bottom={toastSpot().edge === "bottom" ? toastSpot().offset : undefined}
        />
      </If>
    </View>
  );
}
