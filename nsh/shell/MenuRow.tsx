// The application menu drawn in the window, on Windows and on Linux desktops without a Global Menu (chrome_layout.ts): one
// component for both, fed from the same data as the system's menu bar (desktop/menu.ts, menu_model.ts).
//   * MenuRow: row 1 of two. [Windows: the app's icon] eBIRForms File Edit View Window Help Profiles, then drag space (a press
//     moves the window, a double press maximizes it: windowDrag), then room for the window's own buttons (titleBar().right:
//     Windows' caption buttons, GTK's buttons by the desktop's decoration layout; titleBar().left when the layout puts them first).
//     A title opens its menu with Menu.popup under it, in the theme's colours, light and dark.
//   * MenuListButton: the narrow header's ☰ (under 600): every menu as one list with submenus.
//   * bindMenuKeys: F10 puts the keyboard on the row (the first title), Left and Right move along it, Down, Enter or Space
//     open the menu, Esc leaves; Alt with a title's letter opens that menu. Under 600, F10 opens the ☰ list under the ☰.
//     Inside an open menu the platform's own keys work (arrows, Enter, Esc). The row lets go of the keyboard when the menu
//     closes, chosen or dismissed, and its Left, Right, Down, Enter and Space are bound only while it holds the keyboard,
//     never in a text field (the field keeps its own keys).
// Windows' native Win32 menu bar and GTK's PopoverMenuBar are not used: neither shows while the title bar is hidden.
import type { JSX } from "nsh";
import { computed, effect, Image, keymap, signal, untracked, View, type ViewRef } from "nsh";
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
/** The ☰ button's view (the one on screen), to place the list under it. */
let listAnchor: ViewRef | undefined;

/** Just under `view` (where a menu opens below its button), or undefined when it is not on screen (the menu opens at the pointer). */
async function belowAnchor(view: ViewRef | undefined): Promise<{ x: number; y: number } | undefined> {
  const at = view ? await view.measure() : null;
  return at ? { x: at.pagePosition.x, y: at.pagePosition.y + at.bounds.size.height } : undefined;
}

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
  const at = await belowAnchor(anchors.get(title));
  opened.set(title);
  try {
    await Menu.popup(found.items, at);
  } finally {
    if (opened() === title) opened.set(null);
    // However it closed (an item chosen, Esc, a click away), the keyboard goes back to where it was: the row lets go.
    keyed.set(null);
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

/** The narrow header's ☰: every menu as one list, each a submenu, under the ☰ button. */
export async function openMenuList(): Promise<void> {
  const os = menuOs();
  if (!os) return;
  const at = await belowAnchor(listAnchor);
  await Menu.popup(menuNow(os), at);
}

/** The ☰ button (Windows and Linux windows under 600 with an in-window menu). */
export function MenuListButton(props: { testID: string }): JSX.Element {
  return (
    <View ref={(r: ViewRef) => (listAnchor = r)}>
      <IconButton testID={props.testID} icon="menu" tooltip="Menu" onPress={() => void openMenuList()} />
    </View>
  );
}

/** The menu row's keys, for as long as the caller lives (Shell's root). Only where the window draws the menu. */
export function bindMenuKeys(): void {
  // Read through memos: a resize rebinds nothing until the layout itself changes (a breakpoint, the Global Menu).
  const inWindow = computed(() => chromeNow().menu === "window");
  const rowShown = computed(() => chromeNow().menuRow);
  // F10 and Alt+letter, while the menu is the window's own.
  effect(() => {
    if (!inWindow()) return;
    const titles = menus().map((m) => m.title);
    const bindings: Record<string, () => void> = {};
    if (rowShown()) {
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
  // While the keyboard is on the row: Left, Right, Down/Enter/Space, Esc. Only then, and with the default `when`: a text field that
  // has the focus keeps its arrows, Enter and Space (keymap gives them to the field). Esc leaves the row from anywhere.
  const onRow = computed(() => keyed() !== null);
  effect(() => {
    if (!onRow()) return;
    const step = (by: number) => () => {
      const titles = untracked(menus).map((m) => m.title);
      const at = titles.indexOf(keyed() ?? "");
      keyed.set(titles[(at + by + titles.length) % titles.length] ?? null);
    };
    const open = () => {
      const title = keyed();
      if (title) void openMenu(title);
    };
    const unbindMoves = keymap.bind({ left: step(-1), right: step(1), down: open, enter: open, space: open });
    const unbindEscape = keymap.bind("escape", () => keyed.set(null), { when: "always" });
    return () => {
      unbindMoves();
      unbindEscape();
    };
  });
  // The row loses the keyboard when the window's menu is not drawn in it any more.
  effect(() => {
    if (!rowShown()) untracked(() => keyed.set(null));
  });
}
