// The wide title bar (old chrome.js renderTitleBar): 40 points, a window drag region, room for the macOS traffic lights
// and the Windows caption buttons (nsh/desktop titleBar(): the hidden title bar's area). Left: sidebar toggle, back,
// forward, and search and "+" while the sidebar is hidden. Centre: the context pill. Right: bell, theme, panel.
import { keymap, View, windowSize } from "nsh";
import { titleBar } from "nsh/desktop";
import { BREAK, SIZE } from "../theme/tokens";
import { ContextPill, IconButton } from "../ui";
import { app } from "./app";
import { themeIcon } from "./theme";

/** The panel button's words: the panel it would hide or show. */
function panelWord(which: string): string {
  return which === "notifs" ? "notifications" : "tax profile";
}

export function TitleBar() {
  const compact = () => windowSize().width < BREAK.compactPill;
  const tp = () => app.taxpayer();
  const br = () => app.branch();
  const panelTarget = () => app.panel() ?? app.lastPanel();
  return (
    <View testID="title-bar" desktop={{ windowDrag: true }} class="shrink-0 flex-row items-center" style={{ height: SIZE.titleBar }}>
      <View class="min-w-0 flex-1 basis-0 flex-row items-center gap-1" style={{ paddingLeft: 10 + titleBar().left }}>
        <IconButton
          testID="tb-sidebar"
          icon="panel-left"
          tooltip={app.sidebarOpen() ? "Hide sidebar" : "Show sidebar"}
          shortcut={keymap.label("mod+b")}
          onPress={app.toggleSidebar}
        />
        <If when={!compact()}>
          <IconButton testID="tb-back" icon="arrow-left" tooltip="Back" shortcut={keymap.label("mod+[")} disabled={!app.canBack()} onPress={() => app.back()} />
          <IconButton testID="tb-forward" icon="arrow-right" tooltip="Forward" shortcut={keymap.label("mod+]")} disabled={!app.canForward()} onPress={() => app.forward()} />
        </If>
        <If when={!app.sidebarOpen() || app.layout.overlayMode()}>
          <IconButton testID="tb-search" icon="search" tooltip="Search" shortcut={keymap.label("mod+k")} onPress={app.openPalette} />
          <IconButton testID="tb-new" icon="plus" tooltip="New return" onPress={() => app.startNewReturn()} />
        </If>
      </View>
      <View class="min-w-0 flex-none" style={{ maxWidth: compact() ? 160 : 460 }}>
        <ContextPill
          testID="context-pill"
          empty={!tp()}
          open={app.panel() === "profile"}
          compact={compact()}
          name={tp()?.name ?? ""}
          hueKey={tp()?.tin_root ?? ""}
          tin={br()?.tin_display ?? ""}
          branchCode={br()?.code ?? ""}
          onPress={() => app.togglePanel("profile")}
        />
      </View>
      <View class="min-w-0 flex-1 basis-0 flex-row items-center justify-end gap-1" style={{ paddingRight: 10 + titleBar().right }}>
        <IconButton
          testID="tb-notifications"
          icon="bell"
          tooltip="Notifications"
          shortcut={keymap.label("mod+shift+n")}
          pressed={app.panel() === "notifs"}
          dot={app.unreadCount() > 0}
          onPress={() => app.togglePanel("notifs")}
        />
        <IconButton
          testID="tb-theme"
          icon={themeIcon(app.themePref())}
          tooltip={`Theme: ${app.themePref()}, press to change`}
          shortcut={keymap.label("mod+t")}
          onPress={app.cycleTheme}
        />
        <IconButton
          testID="tb-panel"
          icon="panel-right"
          tooltip={`${app.panel() ? "Hide" : "Show"} ${panelWord(panelTarget())}`}
          shortcut={keymap.label(panelTarget() === "notifs" ? "mod+shift+n" : "mod+shift+b")}
          pressed={app.panel() !== null}
          onPress={app.toggleRightPanel}
        />
      </View>
    </View>
  );
}
