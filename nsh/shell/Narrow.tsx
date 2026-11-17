// The narrow layout (under 600 points, and phones): a header with the page's name and the account badge, and a bottom bar of
// four tabs. The badge is the header's one button: it carries a red dot while notifications are unread and opens a menu
// with the tax profile (the taxpayer switcher), search, the notifications and Settings. The header keeps clear of the
// window's own buttons (titleBar()). Back is the platform's back (Android's button); the header's arrow is there for the
// windows that have none (a narrow desktop window).
import type { JSX } from "nsh";
import { safeArea, View } from "nsh";
import { Button } from "nsh/base";
import { titleBar } from "nsh/desktop";
import { SIZE } from "../theme/tokens";
import { Avatar, Icon, IconButton, Menu, MenuItem, MenuSeparator, NavRow, Sheet, Text, TOUCH } from "../ui";
import { app } from "./app";
import { MarkAllRead, NotificationsBody, ProfileBody } from "./Panels";
import { TabButton } from "./parts";
import { ROUTES, TABS } from "./routes";
import { themeIcon } from "./theme";
import type { FormTarget } from "../screens/form/FormPage";

const TAB_VIEWS: string[] = TABS.map((t) => t.view);

/** What the header calls the page: the view's name, or the form code on a return. */
function pageTitle(): string {
  const view = app.view();
  if (view === "form") return `Form ${(app.route().params.target as FormTarget | undefined)?.formCode ?? ""}`;
  return view === "home" ? "eBIRForms" : ROUTES[view].label;
}

export function NarrowBar() {
  return (
    <View
      testID="narrow-bar"
      desktop={{ windowDrag: true }}
      class="shrink-0 flex-row items-center gap-1"
      // The window's own buttons sit at the ends of this bar on the desktops (GTK's, Windows' caption buttons, the
      // traffic lights): pad by their widths so nothing of ours goes under them (0 on phones).
      style={{ minHeight: TOUCH ? 52 : SIZE.titleBar, paddingLeft: 8 + titleBar().left, paddingRight: 8 + titleBar().right, paddingTop: safeArea().top }}
    >
      <If when={app.canBack() && !TAB_VIEWS.includes(app.view())}>
        <IconButton testID="nb-back" icon="arrow-left" tooltip="Back" onPress={() => app.back()} />
      </If>
      <Text testID="nb-title" variant="emph" class="min-w-0 flex-1 pl-1" lines={1}>{pageTitle()}</Text>
      <AccountMenu />
    </View>
  );
}

/** The badge (the taxpayer's avatar, or a person when there is none) and its menu: everything the wide title bar has. */
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
          class={`items-center justify-center rounded-full selected:bg-selected focus-visible:ring-2 focus-visible:ring-accent-blue ${TOUCH ? "size-11" : "size-8"}`}
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
      <MenuItem testID="nb-menu-profile" label={tp()?.name ?? "Account"} icon="user" onSelect={() => app.togglePanel("profile")} />
      <MenuSeparator />
      <MenuItem testID="nb-menu-search" label="Search" icon="search" onSelect={app.openPalette} />
      <MenuItem
        testID="nb-menu-notifications"
        label={unread() > 0 ? `Notifications · ${unread()}` : "Notifications"}
        icon="bell"
        onSelect={() => app.togglePanel("notifs")}
      />
      <MenuItem testID="nb-menu-settings" label="Settings" icon="settings" onSelect={() => app.go("settings")} />
    </Menu>
  );
}

/** Home, Forms, Filings, Calendar. */
export function TabBar() {
  return (
    <View testID="tab-bar" class="shrink-0 flex-row border-t border-border bg-background" style={{ paddingBottom: safeArea().bottom }}>
      <Each of={TABS} key={(t) => t.view}>
        {(t) => <TabButton view={t().view} label={t().label} icon={t().icon} current={app.view() === t().view} onPress={() => app.go(t().view)} />}
      </Each>
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
