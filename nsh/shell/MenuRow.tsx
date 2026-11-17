// The application menu drawn in the window, on Windows and on Linux desktops without a Global Menu (chrome_layout.ts): one
// component for both, fed from the same data as the system's menu bar (desktop/menu.ts, menu_model.ts).
//   * MenuRow: row 1 of two. [Windows: the app's icon] eBIRForms File Edit View Window Help Profiles, then drag space (a press
//     moves the window, a double press maximizes it: windowDrag), then room for the window's own buttons (titleBar().right:
//     Windows' caption buttons, GTK's buttons by the desktop's decoration layout; titleBar().left when the layout puts them first).
//     A title opens its menu with Menu.popup under it, in the theme's colours, light and dark.
//   * MenuListButton: the narrow header's ☰ (under 600): every menu as one list with submenus.
//   * bindMenuKeys: F10 puts the keyboard on the row (the first title), Left and Right move along it, Down, Enter or Space
//     open the menu, Esc leaves; Alt with a title's letter opens that menu. Under 600, F10 opens the ☰ list. Inside an open
//     menu the platform's own keys work (arrows, Enter, Esc).
// Windows' native Win32 menu bar and GTK's PopoverMenuBar are not used: neither shows while the title bar is hidden.
import type { JSX } from "nsh";
import { effect, Image, keymap, signal, untracked, View, type ViewRef } from "nsh";
import { Button } from "nsh/base";
import { Menu, type MenuItem, titleBar } from "nsh/desktop";
import { Icon, ICONS, IconButton, Text } from "../ui";
import { chromeNow, menuOs } from "./chrome";
import { MENU_ROW, menuKey } from "./chrome_layout";
import { menuNow } from "./desktop/menu";

/** The title the keyboard is on (F10), or null. */
const keyed = signal<string | null>(null);
/** The title whose menu is open, or null. */
const opened = signal<string | null>(null);
/** The titles' views, to place a menu under its title. */
const anchors = new Map<string, ViewRef>();

/** The row's menus now: [title, items]. */
function menus(): { title: string; items: MenuItem[] }[] {
  const os = menuOs();
  if (!os) return [];
  return menuNow(os).flatMap((m) => ("label" in m && m.label && m.submenu ? [{ title: m.label, items: m.submenu as MenuItem[] }] : []));
}

/** Opens a title's menu under it; resolves once it closes. */
export async function openMenu(title: string): Promise<void> {
  const found = untracked(menus).find((m) => m.title === title);
  if (!found) return;
  const view = anchors.get(title);
  const at = view ? await view.measure() : null;
  opened.set(title);
  try {
    await Menu.popup(found.items, at ? { x: at.pagePosition.x, y: at.pagePosition.y + at.bounds.size.height } : undefined);
  } finally {
    if (opened() === title) opened.set(null);
  }
}

function Title(props: { title: string; chevron: boolean }): JSX.Element {
  return (
    <Button
      testID={`menu-${props.title}`}
      ref={(r: ViewRef) => anchors.set(props.title, r)}
      accessibilityLabel={props.title + " menu"}
      selected={opened() === props.title || keyed() === props.title}
      class="h-6 flex-row items-center gap-0.5 rounded-(--radius-ctl) px-2 enabled:hover:bg-hover selected:bg-selected focus-visible:ring-2 focus-visible:ring-accent-blue"
      onPress={() => {
        keyed.set(null);
        void openMenu(props.title);
      }}
    >
      <Text variant="body" class={props.title === "eBIRForms" ? "font-semibold" : undefined}>{props.title}</Text>
      <If when={props.chevron}>
        <Icon name="chevron-down" size={12} tone="fg3" />
      </If>
    </Button>
  );
}

/** Row 1: the menu titles, drag space, and room for the window's buttons. */
export function MenuRow(): JSX.Element {
  return (
    <View
      testID="menu-row"
      desktop={{ windowDrag: true }}
      class="shrink-0 flex-row items-center gap-0.5 bg-background"
      style={{ height: MENU_ROW, paddingLeft: 6 + titleBar().left, paddingRight: titleBar().right }}
    >
      <If when={chromeNow().appIcon}>
        <View testID="menu-app-icon" class="items-center justify-center px-1.5">
          <Image src={ICONS.e_logo} style={{ width: 16, height: 16 }} />
        </View>
      </If>
      <Each of={menus()} key={(m) => m.title}>
        {(m) => <Title title={m().title} chevron={chromeNow().chevrons} />}
      </Each>
      {/* The rest of the row is drag space (the row's windowDrag). */}
      <View class="min-w-0 flex-1 self-stretch" />
    </View>
  );
}

/** The narrow header's ☰: every menu as one list, each a submenu. */
export function openMenuList(at?: { x: number; y: number }): Promise<void> {
  const os = menuOs();
  return os ? Menu.popup(menuNow(os), at) : Promise.resolve();
}

/** The ☰ button (Windows and Linux windows under 600 with an in-window menu). */
export function MenuListButton(props: { testID: string }): JSX.Element {
  let view: ViewRef | undefined;
  return (
    <View ref={(r: ViewRef) => (view = r)}>
      <IconButton
        testID={props.testID}
        icon="menu"
        tooltip="Menu"
        onPress={async () => {
          const at = view ? await view.measure() : null;
          await openMenuList(at ? { x: at.pagePosition.x, y: at.pagePosition.y + at.bounds.size.height } : undefined);
        }}
      />
    </View>
  );
}

/** The menu row's keys, for as long as the caller lives (Shell's root). Only where the window draws the menu. */
export function bindMenuKeys(): void {
  // F10 and Alt+letter, while the menu is the window's own.
  effect(() => {
    const layout = chromeNow();
    if (layout.menu !== "window") return;
    const titles = menus().map((m) => m.title);
    const bindings: Record<string, () => void> = {};
    if (layout.menuRow) {
      bindings.f10 = () => keyed.set(keyed() === null ? (titles[0] ?? null) : null);
      const taken: string[] = [];
      for (const title of titles) {
        const key = menuKey(title, taken);
        if (!key) continue;
        taken.push(key);
        bindings["alt+" + key] = () => {
          keyed.set(null);
          void openMenu(title);
        };
      }
    } else {
      bindings.f10 = () => void openMenuList();
    }
    return keymap.bind(bindings, { when: "always" });
  });
  // While the keyboard is on the row: Left, Right, Down/Enter/Space, Esc.
  effect(() => {
    const on = keyed();
    if (on === null) return;
    const step = (by: number) => () => {
      const titles = untracked(menus).map((m) => m.title);
      const at = titles.indexOf(keyed() ?? "");
      keyed.set(titles[(at + by + titles.length) % titles.length] ?? null);
    };
    const open = () => {
      const title = keyed();
      if (title) void openMenu(title);
    };
    return keymap.bind(
      { left: step(-1), right: step(1), down: open, enter: open, space: open, escape: () => keyed.set(null) },
      { when: "always" },
    );
  });
  // The row loses the keyboard when the window's menu is not drawn in it any more.
  effect(() => {
    if (!chromeNow().menuRow) untracked(() => keyed.set(null));
  });
}

/** For tests: the title the keyboard is on, the title whose menu is open. */
export const menuRowState = { keyed: () => keyed(), opened: () => opened() };
