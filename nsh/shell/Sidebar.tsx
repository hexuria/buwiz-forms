// The 248-point sidebar (old chrome.js renderSidebar): search row with the Cmd/Ctrl+K hint and "+", Home, Form library,
// Calendar, Filings, then Recent (the last four returns opened; a pref), and the account row with its menu.
import type { JSX } from "nsh";
import { computed, keymap, Scroll, View } from "nsh";
import { Button } from "nsh/base";
import { frequencyOf, periodLabel, type Frequency } from "../lib/periods";
import { recentOpenOptions, sameReturn, type RecentEntry } from "../lib/recent";
import type { FormTarget } from "../screens/form/FormPage";
import { SIZE } from "../theme/tokens";
import { Avatar, deviceCopy, GroupLabel, Icon, IconButton, Line, Menu, MenuItem, NavRow, TOUCH } from "../ui";
import { app } from "./app";
import { SearchRow } from "./parts";
import { ROUTES, type ViewName } from "./routes";
import { store } from "./store";

const NAV: ViewName[] = ["home", "library", "calendar", "filings"];

/** Only returns whose TIN and branch this session can see: another taxpayer's name never shows to a taxpayer. */
const visibleRecent = computed(() => app.recent().filter((r) => app.branchFor(r.tin)));

/** The return on screen (a stored return or an amendment): its Recent row is marked where it is; opening never moves it. */
const openIdentity = computed(() => {
  const r = app.route();
  if (r.view !== "form") return null;
  const t = r.params.target as FormTarget | undefined;
  if (!t || (t.mode !== "return" && t.mode !== "amend")) return null;
  return { tin: t.tin, code: t.formCode, year: t.year, period: t.period, returnKey: t.key };
});

function recentLabel(r: RecentEntry): string {
  const name = app.taxpayers().find((x) => x.tin_root === String(r.tin).slice(0, 9))?.name ?? "";
  const freq = frequencyOf(store.screenIndex() as { c: string; freq: Frequency }[], r.code);
  return `${r.code} ${periodLabel(freq, r.year, r.period, true)} · ${name}`;
}

/** Who is signed in: the admin (a help desk's own account, or the owner's taxpayer), one taxpayer, or nobody yet. */
const account = computed(() => {
  const own = (root: string | null) => app.taxpayers().find((x) => x.tin_root === root);
  const s = app.session();
  if (app.isAdmin() && app.isHelpdesk()) return { name: "Help desk admin", sub: "TIN 000-000-000", root: "000000000" };
  if (app.isAdmin()) {
    const t = own(app.authState().admin_root);
    return { name: t?.name ?? "Local account", sub: deviceCopy("Admin · this computer"), root: t?.tin_root ?? "account" };
  }
  if (s.kind === "taxpayer") {
    const t = own(s.root);
    return { name: t?.name ?? "Local account", sub: t ? `TIN ${t.tin_display}` : deviceCopy("This computer"), root: s.root };
  }
  return { name: "Local account", sub: deviceCopy("This computer"), root: "account" };
});

/** One Recent row. */
function RecentRow(props: { entry: RecentEntry }) {
  const current = () => {
    const open = openIdentity();
    return open !== null && sameReturn(props.entry, open);
  };
  return (
    <NavRow
      testID={`recent-${String(props.entry.key).replace(/[^A-Za-z0-9]/g, "-")}`}
      leading="file-text"
      label={recentLabel(props.entry)}
      size="recent"
      tooltip={recentLabel(props.entry)}
      current={current()}
      onPress={() => app.openReturn(recentOpenOptions(props.entry))}
    />
  );
}

/** The account row and its menu (Lock app, Account settings, Log out). */
function AccountRow() {
  return (
    <Menu
      testID="account-menu"
      placement="top"
      align="start"
      trigger={(open): JSX.Element => (
        <Button
          testID="account-row"
          accessibilityLabel="Account menu"
          selected={open()}
          class={`w-full flex-row items-center gap-2.5 rounded-(--radius-row) px-2 py-1.5 enabled:hover:bg-hover selected:bg-hover focus-visible:ring-2 focus-visible:ring-accent-blue ${TOUCH ? "min-h-14" : ""}`}
        >
          <Avatar name={account().name} size={28} hueKey={account().root} />
          <View class="min-w-0 flex-1">
            <Line variant="control">{account().name}</Line>
            <Line variant="meta" tone="fg3">{account().sub}</Line>
          </View>
          <Icon name="chevrons-up-down" tone="fg3" />
        </Button>
      )}
    >
      <MenuItem testID="menu-lock" label="Lock app" icon="lock" shortcut={keymap.label("mod+l")} onSelect={app.lockApp} />
      <MenuItem testID="menu-account" label="Account settings" icon="user" shortcut={keymap.label("mod+,")} onSelect={() => app.go("settings")} />
      <MenuItem testID="menu-logout" label="Log out" icon="log-out" onSelect={app.logOut} />
    </Menu>
  );
}

/** The sidebar. In overlay mode (600 to 859 points) the frame floats it over the page; docked it sits beside it. */
export function Sidebar() {
  return (
    <View testID="sidebar" class="h-full min-h-0 shrink-0 bg-background px-2 pt-1 pb-2" style={{ width: SIZE.sidebar }}>
      <View class="flex-row items-center gap-1">
        <SearchRow shortcut={keymap.label("mod+k")} onPress={app.openPalette} />
        <IconButton testID="sb-new" icon="plus" tooltip="New return" onPress={() => app.startNewReturn()} />
      </View>
      {/* The four pages stay out of the scroll: a first row inside a Scroll has no room above it to be brought into view, and a test or a
          screen reader that reaches for it makes the sidebar bounce for two seconds (the "Home takes 2 s" of the real-window runs). */}
      <View testID="sb-nav" class="gap-0.5 pt-3">
        <Each of={NAV} key={(v) => v}>
          {(v) => (
            <NavRow testID={`nav-${v()}`} leading={ROUTES[v()].icon} label={ROUTES[v()].label} current={app.view() === v()} onPress={() => app.go(v())} />
          )}
        </Each>
      </View>
      <Scroll testID="sb-recent" class="min-h-0 flex-1" contentClass="gap-0.5">
        <If when={app.prefs().showRecent && !app.isFresh() && visibleRecent().length > 0}>
          <GroupLabel>Recent</GroupLabel>
          <Each of={visibleRecent()} key={(r) => r.key}>{(r) => <RecentRow entry={r()} />}</Each>
        </If>
      </Scroll>
      <View testID="sb-foot" class="pt-2">
        <AccountRow />
      </View>
    </View>
  );
}
