// The narrow layout (every phone and tablet, at any width; a desktop window under 600 points): a header and a bottom bar of four
// tabs. The header: Back on a pushed page, the page's name, New return (+), Search, and the account badge; on a tablet the open
// taxpayer's name, TIN and branch beside the badge, so whose return is on screen is always in view. The badge carries a red dot
// while notifications are unread and opens a menu with the notifications, Settings, Lock app and Log out (search is the header's
// own button, the tax profile the palette's). The header keeps clear of the window's own buttons (titleBar()) and of a phone's
// sensor housing turned sideways.
import type { JSX } from "nsh";
import { safeArea, View } from "nsh";
import { Button } from "nsh/base";
import { titleBar } from "nsh/desktop";
import { SIZE } from "../theme/tokens";
import { Avatar, Icon, IconButton, Menu, MenuItem, MenuSeparator, NavRow, Sheet, T_SIZE, tabletRoom, Text, TOUCH } from "../ui";
import { app } from "./app";
import { chromeNow } from "./chrome";
import { MenuListButton } from "./MenuRow";
import { MarkAllRead, NotificationsBody, ProfileBody } from "./Panels";
import { TabButton } from "./parts";
import { ROUTES, TABS } from "./routes";
import { themeIcon } from "./theme";
import type { FormTarget } from "../screens/form/FormPage";
import { backToSettingsList, sectionTitle } from "../screens/settings";

const TAB_VIEWS: string[] = TABS.map((t) => t.view);

/** The Settings section the route names (its own page in this layout), or null. */
function settingsSection(): string | null {
  const section = app.view() === "settings" ? app.params().section : null;
  return typeof section === "string" ? section : null;
}

/** What the header calls the page: the view's name, the form code on a return, the section on a page of Settings. */
function pageTitle(): string {
  const view = app.view();
  if (view === "form") return `Form ${(app.route().params.target as FormTarget | undefined)?.formCode ?? ""}`;
  const section = settingsSection();
  if (section) return sectionTitle(section);
  return view === "home" ? "eBIRForms" : ROUTES[view].label;
}

/** Back from the header: one step of history; a Settings section opened by name has none behind it and goes to the list. */
function headerBack(): void {
  if (settingsSection()) backToSettingsList();
  else app.back();
}

export function NarrowBar() {
  return (
    <View
      testID="narrow-bar"
      desktop={{ windowDrag: true }}
      class="shrink-0 flex-row items-center gap-1"
      // The window's own buttons sit at the ends of this bar on the desktops (GTK's, Windows' caption buttons, the
      // traffic lights): pad by their widths so nothing of ours goes under them (0 on phones).
      style={{
        minHeight: TOUCH ? 52 : SIZE.titleBar,
        paddingLeft: 8 + titleBar().left + safeArea().left,
        paddingRight: 8 + titleBar().right + safeArea().right,
        paddingTop: safeArea().top,
      }}
    >
      {/* Windows and Linux windows whose menu is drawn in the window: ☰ lists every menu (macOS and KDE keep theirs at the top of the screen). */}
      <If when={chromeNow().hamburger}>
        <MenuListButton testID="nb-menu" />
      </If>
      <If when={(app.canBack() || settingsSection() !== null) && !TAB_VIEWS.includes(app.view())}>
        <IconButton testID="nb-back" icon="arrow-left" tooltip="Back" onPress={headerBack} />
      </If>
      <Text testID="nb-title" variant="emph" class="min-w-0 flex-1 pl-1" lines={1}>{pageTitle()}</Text>
      <IconButton testID="nb-new" icon="plus" tooltip="New return" onPress={() => app.startNewReturn()} />
      <IconButton testID="nb-search" icon="search" tooltip="Search" onPress={app.openPalette} />
      <If when={tabletRoom() && app.taxpayer()}>
        <TaxpayerLabel />
      </If>
      <AccountMenu />
    </View>
  );
}

/**
 * A tablet's header: whose return is on screen, the taxpayer's name over its TIN and branch (the wide title bar's context pill, said
 * the narrow way). A press opens the tax profile sheet, where the taxpayer and the branch are switched.
 */
function TaxpayerLabel() {
  const tp = () => app.taxpayer();
  const br = () => app.branch();
  // The TIN with its branch code (000-123-453-00000), then the branch's name.
  const line = () => [br()?.tin_display, br()?.name].filter(Boolean).join(" · ");
  return (
    <Button
      testID="nb-taxpayer"
      accessibilityLabel={`Tax profile: ${tp()?.name ?? ""}`}
      class="min-w-0 shrink justify-center rounded-(--radius-row) px-2 py-1 enabled:hover:bg-hover active:bg-selected focus-visible:ring-2 focus-visible:ring-accent-blue"
      style={{ maxWidth: 320 }}
      onPress={() => app.togglePanel("profile")}
    >
      <Text testID="nb-taxpayer-name" variant="emph" class="text-right" lines={1}>{tp()?.name ?? ""}</Text>
      <Text testID="nb-taxpayer-line" variant="cap" tone="fg3" class="text-right" lines={1}>{line()}</Text>
    </Button>
  );
}

/** The badge (the taxpayer's avatar, or a person when there is none) and its menu: the session's actions, as the wide account row's. */
function AccountMenu() {
  const tp = () => app.taxpayer();
  const unread = () => app.unreadCount();
  return (
    <Menu
      testID="nb-account-menu"
      placement="bottom"
      align="end"
      trigger={(open): JSX.Element => (
        <Button
          testID="nb-avatar"
          accessibilityLabel={unread() > 0 ? `Account, ${unread()} unread notifications` : "Account"}
          selected={open() || app.panel() !== null}
          class={`items-center justify-center rounded-full selected:bg-selected focus-visible:ring-2 focus-visible:ring-accent-blue ${TOUCH ? T_SIZE : "size-8"}`}
        >
          <If when={tp()} else={<Icon name="user" tone="fg2" />}>
            <Avatar name={tp()?.name ?? ""} size={28} hueKey={tp()?.tin_root} />
          </If>
          <If when={unread() > 0}>
            <View testID="nb-unread-dot" class={`absolute size-2.5 rounded-full border-2 border-background bg-danger ${TOUCH ? "top-1.5 right-1.5" : "top-0 right-0"}`} />
          </If>
        </Button>
      )}
    >
      <MenuItem
        testID="nb-menu-notifications"
        label={unread() > 0 ? `Notifications · ${unread()}` : "Notifications"}
        icon="bell"
        onSelect={() => app.togglePanel("notifs")}
      />
      <MenuItem testID="nb-menu-settings" label="Settings" icon="settings" onSelect={() => app.go("settings")} />
      <MenuSeparator />
      <MenuItem testID="nb-menu-lock" label="Lock app" icon="lock" onSelect={app.lockApp} />
      <MenuItem testID="nb-menu-logout" label="Log out" icon="log-out" onSelect={app.logOut} />
    </Menu>
  );
}

/** The four tabs together are never wider than this: on a tablet they sit grouped in the middle, about 200 points each. */
const TABS_MAX_WIDTH = 800;

/** Home, Forms, Filings, Calendar. */
export function TabBar() {
  return (
    <View
      testID="tab-bar"
      class="shrink-0 items-center border-t border-border bg-background"
      style={{ paddingBottom: safeArea().bottom, paddingLeft: safeArea().left, paddingRight: safeArea().right }}
    >
      <View testID="tab-row" class="w-full flex-row" style={{ maxWidth: TABS_MAX_WIDTH }}>
        <Each of={TABS} key={(t) => t.view}>
          {(t) => <TabButton view={t().view} label={t().label} icon={t().icon} current={app.view() === t().view} onPress={() => app.go(t().view)} />}
        </Each>
      </View>
    </View>
  );
}

/** The account sheet's end: Settings, the theme, Lock app, Log out (the wide layout has these in the account menu and the title bar). */
function SheetExtras() {
  return (
    <View testID="sheet-extras" class="gap-0.5 border-t border-border pt-2">
      <NavRow testID="sheet-settings" leading="settings" label="Settings" onPress={() => app.go("settings")} />
      <NavRow testID="sheet-theme" leading={themeIcon(app.themePref())} label={`Theme: ${app.themePref()}`} onPress={app.cycleTheme} />
      <NavRow testID="sheet-lock" leading="lock" label="Lock app" onPress={app.lockApp} />
      <NavRow testID="sheet-logout" leading="log-out" label="Log out" onPress={app.logOut} />
    </View>
  );
}

/** The panels as sheets: the tax profile (from the avatar) and the notifications (from the bell). */
export function PanelSheets() {
  return (
    <>
      <Sheet testID="sheet-profile" open={app.panel() === "profile"} onOpenChange={(o) => !o && app.closePanel()} title="Tax profile">
        <ProfileBody />
        <SheetExtras />
      </Sheet>
      <Sheet
        testID="sheet-notifications"
        open={app.panel() === "notifs"}
        onOpenChange={(o) => !o && app.closePanel()}
        title="Notifications"
        footer={<MarkAllRead />}
      >
        <NotificationsBody />
      </Sheet>
    </>
  );
}
