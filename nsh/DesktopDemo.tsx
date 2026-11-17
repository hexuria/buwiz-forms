// A test fixture, not a screen: the frame with the desktop host (application menu, tray, hide-to-tray, hotkey, updater) over the demo
// taxpayer, for shell/desktop/desktop.test.tsx and shots/desktop.test.tsx. What a test asks for goes in open(entry, { host: { params } }):
//   demo     default true: the demo taxpayer (Juana Santos) with the current tax year; false: an empty database (no taxpayer)
//   locked   true: the account gate is in front (the filing menus go)
//   start    the dev route ("form=1601C/2025/3")
//   poll     true: run the frame's polls (the update status is read every second)
//   updater  "found": a fake nsh/updater that finds 0.3.0 | "current": one that finds nothing | absent: the real one (no source configured)
//   hotkey   a combo stored as the global hotkey before the host starts ("control+alt+shift+9")
//   system   "fake": GlobalShortcut is a fake that takes every chord (the "r-chords" readout lists what is registered; a test presses one
//            with host.send("run", { id: "chord:ctrl+alt+shift+9" }) and changes the stored combo with { id: "hotkey:control+alt+B" });
//            absent: the real GlobalShortcut (in a window of a real platform the system answers)
//   solo     true: "Just me" is chosen for the demo taxpayer, so the admin sees Settings > Computer (start "section=computer")
//   refuse   a chord the fake GlobalShortcut does not take, as the system does for one another app holds ("ctrl+alt+b")
//   signOut  true: the Buwiz account's sign-out answers signed out at once (no network) and the gate then stays closed; the readout
//            "r-settings-search" shows Settings' search text
// Over the frame, a strip of 8-point buttons (off the window's rounded corner, where a click misses the window) runs the installed menu's items (id "run:File > New Return…") and the tray's ("tray:Show
// eBIRForms"), because a test cannot click the system's menu bar; the readouts say what the menu did. While the window is hidden nothing
// on it can be pressed or read: a test sends host.send("run", { id }) and reads app.window("main").state(). It sits at the nsh root because a
// real-window run serves assets from the entry's folder down.
import { host, mount, signal, Text, View } from "nsh";
import { Button } from "nsh/base";
import { currentWindow } from "nsh/desktop";
import "./theme/fonts";
import { bir } from "./bir";
import { createDemoTaxpayer, warmForms } from "./bir/testing";
import { nav } from "./screens/settings/nav";
import { app } from "./shell/app";
import { store } from "./shell/store";
import { DesktopHost, flatten, hotkeyStatus, installedMenu, installedTray, type UpdaterApi } from "./shell/desktop";
import { Shell } from "./shell/Shell";

// A start route that opens a form builds every form's schema in the frame's first render (the host does it lazily, once per process): more than the
// 2 s a headless run gives one JS call on a slow machine, where a bundle's load has 10 s.
if (/form=/.test(String((host.params as { start?: string } | null)?.start ?? ""))) warmForms();

interface Params {
  demo?: boolean;
  locked?: boolean;
  start?: string;
  poll?: boolean;
  updater?: "found" | "current";
  hotkey?: string;
  system?: "fake";
  solo?: boolean;
  refuse?: string;
  signOut?: boolean;
}

/** What the fake updater was asked, in order. */
const asked = signal<string[]>([]);
/** The chords the fake GlobalShortcut holds, with what pressing them runs. */
const chords = signal<Record<string, () => void>>({});

/** The chords the fake GlobalShortcut refuses (another app holds them). */
const refusing = new Set<string>();

/** A GlobalShortcut that takes every chord but the ones another app holds. */
const fakeShortcuts = {
  register: async (shortcut: string, onPress: () => void) => {
    if (refusing.has(shortcut)) return false;
    chords.update((held) => ({ ...held, [shortcut]: onPress }));
    return true;
  },
  unregister: (shortcut: string) =>
    chords.update((held) => {
      const { [shortcut]: _gone, ...rest } = held;
      return rest;
    }),
};

function fakeUpdater(mode: "found" | "current"): UpdaterApi {
  const note = (what: string) => asked.update((list) => [...list, what]);
  return {
    configured: () => true,
    check: async () => {
      note("check");
      return mode === "found" ? { version: "0.3.0" } : null;
    },
    download: async () => note("download"),
    install: async () => note("install"),
  };
}

/** What the system's menu bar and tray would offer now: the installed menu's items and the tray's, by id. */
function runnable() {
  return [
    ...flatten(installedMenu()).flatMap((l) => (l.select ? [{ id: "run:" + l.path, select: l.select }] : [])),
    ...flatten(installedTray()).flatMap((l) => (l.select ? [{ id: "tray:" + l.path, select: l.select }] : [])),
  ];
}

function Runner() {
  const items = runnable;
  return (
    <View testID="runner" class="absolute bottom-3 left-16 flex-row flex-wrap" style={{ width: 400 }}>
      <Each of={items()} key={(i) => i.id}>
        {(i) => <Button testID={i().id} class="size-2 bg-primary" onPress={() => i().select()} />}
      </Each>
    </View>
  );
}

function Demo() {
  const p = host.params as Params;
  if (p.demo !== false) {
    const taxpayer = createDemoTaxpayer();
    const head = taxpayer.branches[0];
    if (bir.list_tax_years(head.tin).length === 0) {
      const year = new Date().getFullYear();
      bir.add_tax_year(head.tin, year, false);
      bir.set_yearly_forms(head.tin, year, ["1601C", "2551Q"]);
    }
  }
  if (p.solo && bir.auth_state().mode === null) bir.setup_solo(createDemoTaxpayer().tin_root);
  if (p.refuse) refusing.add(p.refuse);
  if (p.hotkey) bir.set_global_hotkey(p.hotkey);
  if (p.locked) app.setCloudLocked(true);
  if (p.signOut) {
    const h = bir as unknown as Record<string, unknown>;
    let signedOut = false;
    h.cloud_account_sign_out = async () => {
      signedOut = true;
      return { server_url: "http://localhost:3008", server_url_source: "build", signed_in: false, email: null, user_id: null, offline: false, detail: "" };
    };
    const realGate = bir.cloud_account_gate;
    h.cloud_account_gate = () => (signedOut ? { unlocked: false, offline: false, server_url: "http://localhost:3008", email: null, detail: "" } : realGate());
  }
  const win = currentWindow();
  // A hidden window shows nothing a test can press: the test sends "run" to choose an item while the window is hidden, as the tray
  // (the system's) is chosen from outside the window. (await app.host.send("run", { id: "tray:Show eBIRForms" }))
  host.on("run", (payload) => {
    const id = String((payload as { id?: unknown } | null)?.id ?? "");
    if (id.startsWith("chord:")) chords()[id.slice(6)]?.();
    else if (id.startsWith("hotkey:")) store.write("set_global_hotkey", id.slice(7));
    else
      runnable()
        .find((item) => item.id === id)
        ?.select();
  });
  return (
    <View class="flex-1">
      <Shell start={p.start} polling={p.poll === true} />
      <DesktopHost updater={p.updater ? fakeUpdater(p.updater) : undefined} shortcuts={p.system === "fake" ? fakeShortcuts : undefined} />
      <View class="absolute top-12 right-0 items-end">
        <Text testID="r-view">{"view=" + app.view()}</Text>
        <Text testID="r-overlay">{"overlay=" + (app.overlay() ?? "none")}</Text>
        <Text testID="r-panel">{"panel=" + (app.panel() ?? "none")}</Text>
        <Text testID="r-sidebar">{"sidebar=" + (app.sidebarOpen() ? "open" : "closed")}</Text>
        <Text testID="r-theme">{"theme=" + app.themePref()}</Text>
        <Text testID="r-visible">{"visible=" + String(win.state()?.visible)}</Text>
        <Text testID="r-hotkey">{"hotkey=" + hotkeyStatus()}</Text>
        <Text testID="r-chords">{"chords=" + Object.keys(chords()).join(",")}</Text>
        <Text testID="r-update">{"update=" + (app.updateStatus()?.phase ?? "none")}</Text>
        <Text testID="r-asked">{"asked=" + asked().join(",")}</Text>
        <Text testID="r-menu">{"menu=" + flatten(installedMenu()).length}</Text>
        <Text testID="r-settings-search">{"settings search=" + nav.query()}</Text>
        <Text testID="r-gate">{"gate=" + (app.cloudLocked() ? "closed" : "open")}</Text>
      </View>
      <Runner />
    </View>
  );
}

mount(Demo);
