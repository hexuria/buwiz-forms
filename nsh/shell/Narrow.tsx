// The narrow layout (under 600 points, and phones): a header with the page's name, search, the bell and the avatar (which
// opens a sheet with the taxpayer switcher, the profile links and Settings), and a bottom bar of four tabs. Back is the
// platform's back (Android's button); the header's arrow is there for the windows that have none (a narrow desktop window).
import { safeArea, View } from "nsh";
import { Button } from "nsh/base";
import { titleBar } from "nsh/desktop";
import { SIZE } from "../theme/tokens";
import { Avatar, IconButton, NavRow, Sheet, Text, TOUCH } from "../ui";
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
  const tp = () => app.taxpayer();
  return (
    <View
      testID="narrow-bar"
      desktop={{ windowDrag: true }}
      class="shrink-0 flex-row items-center gap-1 pr-2"
      style={{ minHeight: TOUCH ? 52 : SIZE.titleBar, paddingLeft: 8 + titleBar().left, paddingTop: safeArea().top }}
    >
      <If when={app.canBack() && !TAB_VIEWS.includes(app.view())}>
        <IconButton testID="nb-back" icon="arrow-left" tooltip="Back" onPress={() => app.back()} />
      </If>
      <Text testID="nb-title" variant="emph" class="min-w-0 flex-1 pl-1" lines={1}>{pageTitle()}</Text>
      <IconButton testID="nb-search" icon="search" tooltip="Search" onPress={app.openPalette} />
      <IconButton testID="nb-notifications" icon="bell" tooltip="Notifications" pressed={app.panel() === "notifs"} dot={app.unreadCount() > 0} onPress={() => app.togglePanel("notifs")} />
      <If when={tp()} else={<IconButton testID="nb-avatar" icon="user" tooltip="Account" onPress={() => app.togglePanel("profile")} />}>
        <Button
          testID="nb-avatar"
          accessibilityLabel="Account"
          selected={app.panel() === "profile"}
          class={`items-center justify-center rounded-full selected:bg-selected focus-visible:ring-2 focus-visible:ring-accent-blue ${TOUCH ? "size-11" : "size-8"}`}
          onPress={() => app.togglePanel("profile")}
        >
          <Avatar name={tp()?.name ?? ""} size={28} hueKey={tp()?.tin_root} />
        </Button>
      </If>
    </View>
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
