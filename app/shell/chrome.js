// The shell chrome (DESIGN_SYSTEM.md "Shell layout"): the 40 px title bar,
// the 248 px sidebar with the account menu, the 320 px right panel (tax
// profile or notifications), the command palette and the start-a-return
// dialog. No separators in the title bar or the panels: spacing groups.
//
// Windows draws minimize, maximize, and close on this title bar. Linux draws
// them on the menu row instead (one row with the menus). macOS uses the
// system traffic lights.

import { div } from "gpui-kit";
import {
  tokens,
  SIZE,
  SPACE,
  RADIUS,
  BREAK,
  Text,
  Line,
  Code,
  Icon,
  IconButton,
  AppButton,
  NavRow,
  GroupLabel,
  Kbd,
  Avatar,
  StepCircle,
  ContextPill,
  Chip,
  SidePanel,
  Overlay,
  Dropdown,
  TextField,
  DigitCells,
} from "./ui.js";
import { themeIcon } from "./theme.js";
import { recentOpenOptions } from "./recent.js";
import { formatTin, shortDate } from "./store.js";
import { periodOptions, periodLabel, frequencyOf } from "./periods.js";

// ---------------------------------------------------------------- title bar

function captionButtons(cx) {
  const t = tokens(cx);
  const button = (id, icon, label, onClick, close, area) =>
    div()
      .id(id)
      .window_control_area(area)
      .flex()
      .items_center()
      .justify_center()
      .w(SIZE.captionButton)
      .h_full()
      .accessibility_label(label)
      .hover((s) => (close ? s.bg(t.captionClose) : s.bg(t.hover)))
      .on_click((_e, cx) => onClick(cx))
      .child(Icon(cx, icon, { size: 10, color: "fg" }));
  return div()
    .flex()
    .flex_row()
    .h_full()
    .ml(SPACE[1])
    .child(button("caption-minimize", "minus", "Minimize", () => window.minimize_window(), false, "min"))
    .child(button("caption-maximize", "square", "Maximize", () => window.zoom_window(), false, "max"))
    .child(
      button(
        "caption-close",
        "x",
        "Close",
        // Hide to the menu-bar tray. Quit is the application menu (Cmd/Ctrl+Q),
        // which stops the submission worker and releases the database lock.
        () => window.dispatch_action("hide-to-tray"),
        true,
        "close",
      ),
    );
}

/** Title bar: left controls, the context pill centred, notifications + theme right. */
export function renderTitleBar(cx, app, L) {
  const compact = L.vp.width < BREAK.compactPill;
  const left = div().flex().flex_row().flex_1().flex_basis(0).min_w(0).items_center().gap(SPACE[1]).pl(10);
  if (app.isMac) left.child(div().w(52 + 14).flex_none()); // traffic lights (system-drawn)
  left.child(
    IconButton(cx, "tb-sidebar", "panel-left", {
      tooltip: app.sidebarOpen ? "Hide sidebar" : "Show sidebar",
      pressed: false,
      onClick: (_e, cx) => app.toggleSidebar(cx),
    }),
  );
  if (!compact) {
    left.child(
      IconButton(cx, "tb-back", "arrow-left", {
        tooltip: "Back",
        disabled: !app.history.length,
        onClick: (_e, cx) => app.goBack(cx),
      }),
    );
    left.child(
      IconButton(cx, "tb-forward", "arrow-right", {
        tooltip: "Forward",
        disabled: !app.forward.length,
        onClick: (_e, cx) => app.goForward(cx),
      }),
    );
  }
  // Search and new-return show here only while the sidebar is hidden.
  if (!app.sidebarOpen || L.narrow) {
    left.child(IconButton(cx, "tb-search", "search", { tooltip: "Search", onClick: (_e, cx) => app.openPalette(cx) }));
    left.child(IconButton(cx, "tb-new", "plus", { tooltip: "New return", onClick: (_e, cx) => app.startNewReturn(cx) }));
  }

  const t = app.taxpayer();
  const b = app.branch();
  const pill = ContextPill(cx, "context-pill", {
    empty: !t,
    open: app.panel === "profile",
    compact,
    name: t ? t.name : "",
    hueKey: t ? t.tin_root : "",
    tin: t && b ? formatTin(t.tin_root, b.code) : "",
    branchCode: b ? b.code : "",
    onClick: (_e, cx) => app.togglePanel("profile", cx),
  });

  const unread = app.notifications().some((n) => n.unread);
  const right = div()
    .flex()
    .flex_row()
    .flex_1()
    .flex_basis(0)
    .min_w(0)
    .justify_end()
    .items_center()
    .gap(SPACE[1])
    .h_full()
    .when(app.isMac, (el) => el.pr(10))
    .child(
      IconButton(cx, "tb-notifications", "bell", {
        tooltip: "Notifications",
        pressed: app.panel === "notifs",
        dot: unread,
        onClick: (_e, cx) => app.togglePanel("notifs", cx),
      }),
    )
    .child(
      IconButton(cx, "tb-theme", themeIcon(app.themePref), {
        tooltip: `Theme: ${app.themePref} (click to change)`,
        onClick: (_e, cx) => app.cycleTheme(cx),
      }),
    )
    .child(
      IconButton(cx, "tb-panel", "panel-right", {
        tooltip: app.panel
          ? `Hide ${app.panel === "notifs" ? "notifications" : "tax profile"}`
          : `Show ${app.lastPanel === "notifs" ? "notifications" : "tax profile"}`,
        pressed: Boolean(app.panel),
        onClick: (_e, cx) => app.toggleRightPanel(cx),
      }),
    );
  // Linux caption buttons are on the menu row. Drawing them here too would
  // stack a second set next to the sidebar icons.
  if (!app.isMac && !app.isLinux) right.child(captionButtons(cx));

  return div()
    .id("title-bar")
    .flex()
    .flex_row()
    .flex_none()
    .items_center()
    .w_full()
    .h(SIZE.titleBar)
    // Native drag area: pressing and dragging moves the window, and a
    // double-click zooms it (or what the user set on macOS).
    .window_control_area("drag")
    .child(left)
    .child(div().flex().flex_none().min_w(0).max_w(compact ? 160 : 460).child(pill))
    .child(right);
}

/**
 * The title bar of the sign-in and lock screens: a drag area, the theme
 * button and (off macOS) the caption buttons. Nothing names a taxpayer.
 */
export function renderBareTitleBar(cx, app) {
  const right = div()
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[1])
    .h_full()
    .when(app.isMac, (el) => el.pr(10))
    .child(
      IconButton(cx, "tb-theme", themeIcon(app.themePref), {
        tooltip: `Theme: ${app.themePref} (click to change)`,
        onClick: (_e, cx) => app.cycleTheme(cx),
      }),
    );
  if (!app.isMac && !app.isLinux) right.child(captionButtons(cx));
  return div()
    .id("title-bar")
    .flex()
    .flex_row()
    .flex_none()
    .items_center()
    .justify_end()
    .w_full()
    .h(SIZE.titleBar)
    .window_control_area("drag")
    .child(right);
}

// ---------------------------------------------------------------- sidebar

export function renderSidebar(cx, app) {
  const t = tokens(cx);
  const kbd = app.isMac ? "⌘K" : "Ctrl K";
  const search = div()
    .id("sb-search")
    .flex()
    .flex_row()
    .flex_1()
    .min_w(0)
    .items_center()
    .gap(SPACE[2])
    .h(SIZE.control)
    .pl(10)
    .pr(SPACE[2])
    .rounded(RADIUS.row)
    .border(1)
    .border_color(t.border)
    .bg(t.surface)
    .cursor_pointer()
    .hover((s) => s.border_color(t.borderStrong))
    .accessibility_label("Search")
    .on_click((_e, cx) => app.openPalette(cx))
    .child(Icon(cx, "search", { color: "fg3" }))
    .child(Line(cx, "body", "Search", "fg3").flex_1())
    .child(Kbd(cx, kbd));

  const top = div()
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[1])
    .child(search)
    .child(IconButton(cx, "sb-new", "plus", { tooltip: "New return", onClick: (_e, cx) => app.startNewReturn(cx) }));

  const nav = div().id("sb-nav").flex().flex_col().gap(2).pt(SPACE[3]);
  for (const [view, icon, label] of [
    ["home", "house", "Home"],
    ["library", "file-text", "Form library"],
    ["calendar", "calendar", "Calendar"],
    ["filings", "history", "Filings"],
  ]) {
    nav.child(NavRow(cx, `nav-${view}`, icon, label, { current: app.view === view, onClick: (_e, cx) => app.go(view, cx) }));
  }
  // Only returns whose TIN and branch this session can see: another
  // taxpayer's name never shows to a taxpayer.
  const recent = app.recent.filter((r) => app.branchFor(r.tin));
  if (app.prefs.showRecent && !app.isFresh() && recent.length) {
    nav.child(GroupLabel(cx, "Recent"));
    recent.forEach((r) => {
      const name = (app.taxpayers().find((x) => x.tin_root === String(r.tin).slice(0, 9)) || {}).name || "";
      const label = `${r.code} ${periodLabel(frequencyOf(app.store.screenIndex(), r.code), r.year, r.period, true)} · ${name}`;
      nav.child(
        NavRow(cx, `recent-${String(r.key).replace(/[^A-Za-z0-9]/g, "-")}`, "file-text", label, {
          tooltip: label,
          onClick: (_e, cx) => app.openForm(recentOpenOptions(r), cx),
        }),
      );
    });
  }

  // Settings lives in the account menu (Account settings), not as its own row.
  const foot = div().id("sb-foot").relative().flex().flex_col().gap(2).pt(SPACE[2]);
  foot.child(accountRow(cx, app));

  return div()
    .id("sidebar")
    .flex()
    .flex_col()
    .flex_none()
    .w(SIZE.sidebar)
    .h_full()
    .min_h(0)
    .pt(SPACE[1])
    .px(SPACE[2])
    .pb(SPACE[2])
    .child(top)
    .child(div().id("sb-scroll").flex_1().min_h(0).overflow_y_scroll().overflow_x_hidden().child(nav))
    .child(foot);
}

/** The app account row and its menu (Lock app, Account settings, Log out). */
function accountRow(cx, app) {
  const t = tokens(cx);
  // Who is signed in: the admin (a help desk's own account, or the owner's
  // taxpayer), one taxpayer, or nobody yet on a computer without a PIN.
  const a = app.authState();
  const own = (root) => app.taxpayers().find((x) => x.tin_root === root);
  let name = "Local account";
  let sub = "This computer";
  if (app.isAdmin() && app.isHelpdesk()) {
    name = "Help desk admin";
    sub = "TIN 000-000-000";
  } else if (app.isAdmin()) {
    name = (own(a.admin_root) || {}).name || name;
    sub = "Admin · this computer";
  } else if (app.session.kind === "taxpayer") {
    const t = own(app.session.root);
    name = (t || {}).name || name;
    sub = t ? `TIN ${t.tin_display}` : sub;
  }
  const row = div()
    .id("account-row")
    .flex()
    .flex_row()
    .items_center()
    .gap(10)
    .w_full()
    .py(6)
    .px(SPACE[2])
    .rounded(RADIUS.row)
    .cursor_pointer()
    .when(app.accountMenu, (el) => el.bg(t.hover))
    .hover((s) => s.bg(t.hover))
    .accessibility_label("Account menu")
    .on_click((_e, cx) => {
      app.accountMenu = !app.accountMenu;
      cx.notify();
    })
    .child(Avatar(cx, name, 28, "account"))
    .child(
      div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(0)
        .child(Line(cx, "control", name))
        .child(Line(cx, "meta", sub, "fg3")),
    )
    .child(Icon(cx, "chevrons-up-down", { color: "fg3" }));

  const wrap = div().relative().w_full().child(row);
  if (app.accountMenu) {
    // Paints above everything in the sidebar footer, Settings included.
    wrap.child(
      div()
        .id("account-menu")
        .absolute()
        .bottom(56 + 4)
        .left(0)
        .right(0)
        .flex()
        .flex_col()
        .gap(2)
        .p(SPACE[1])
        .rounded(RADIUS.card)
        .border(1)
        .border_color(t.border)
        .bg(t.raised)
        .shadow_xl()
        .on_mouse_down_out((_e, cx) => {
          app.accountMenu = false;
          cx.notify();
        })
        .child(div().px(SPACE[2]).py(SPACE[2]).mb(SPACE[1]).border_b(1).border_color(t.border).child(Text(cx, "meta", "Signed in on this computer", "fg3")))
        .child(NavRow(cx, "menu-lock", "lock", "Lock app", { kbd: app.isMac ? "⌘L" : "Ctrl L", onClick: (_e, cx) => app.lockApp(cx) }))
        .child(NavRow(cx, "menu-account", "user", "Account settings", { onClick: (_e, cx) => app.go("settings", cx) }))
        .child(NavRow(cx, "menu-logout", "log-out", "Log out", { onClick: (_e, cx) => app.logOut(cx) })),
    );
  }
  return wrap;
}

// ---------------------------------------------------------------- right panel

export function renderRightPanel(cx, app) {
  return app.panel === "notifs" ? notificationsPanel(cx, app) : profilePanel(cx, app);
}

function notificationsPanel(cx, app) {
  const list = app.notifications();
  const body = div().flex().flex_col().gap(2);
  if (!list.length) {
    body.child(div().py(40).flex().justify_center().child(Text(cx, "body", "No notifications yet.", "fg3")));
  }
  list.forEach((n, i) => {
    const t = tokens(cx);
    const tone = n.tone === "warning" ? { bg: t.warningSoft, fg: "warning" } : n.tone === "success" ? { bg: t.successSoft, fg: "success" } : { bg: t.hover, fg: "fg2" };
    const when = div().flex().flex_col().items_end().gap(6).flex_none().child(Text(cx, "cap", shortDate(n.when), "fg3"));
    if (n.unread) when.child(div().size(6).rounded(RADIUS.full).bg(t.accent));
    body.child(
      div()
        .id(`notif-${i}`)
        .flex()
        .flex_row()
        .gap(SPACE[3])
        .py(10)
        .px(SPACE[2])
        .rounded(RADIUS.row)
        .hover((s) => s.bg(t.hover))
        .child(div().flex().flex_none().items_center().justify_center().size(28).rounded(RADIUS.full).bg(tone.bg).child(Icon(cx, n.icon, { color: tone.fg })))
        .child(
          div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(0)
            .child(Text(cx, "control", n.title).whitespace_normal())
            .child(Text(cx, "meta", n.sub, "fg3").whitespace_normal()),
        )
        .child(when),
    );
  });
  return SidePanel(cx, "panel-notifications", {
    title: "Notifications",
    action: AppButton(cx, "notif-mark-all", "Mark all read", {
      variant: "ghost",
      size: "sm",
      disabled: !list.some((n) => n.unread),
      onClick: (_e, cx) => app.markAllRead(cx),
    }),
    body,
  });
}

function profilePanel(cx, app) {
  const t = tokens(cx);
  const tp = app.taxpayer();
  if (!tp) {
    const body = div()
      .flex()
      .flex_col()
      .items_center()
      .gap(SPACE[2])
      .pt(SPACE[8])
      .px(SPACE[2])
      .child(StepCircle(cx, Icon(cx, "user", { color: "fg2" }), 56))
      .child(Text(cx, "title", app.taxpayers().length ? "No taxpayer open" : "No taxpayer yet").mt(SPACE[1]))
      .child(
        Text(
          cx,
          "meta",
          app.taxpayers().length
            ? "Choose one on Home or with search. The one you open shows here, with its branches, tax years, forms and templates."
            : "The taxpayer you file for shows here, with its branches, tax years, forms and templates.",
          "fg2",
        )
          .text_center()
          .max_w(240)
          .whitespace_normal(),
      )
      .when(app.canAddTaxpayer(), (el) =>
        el.child(
          AppButton(cx, "panel-new-taxpayer", "New taxpayer", {
            variant: "primary",
            size: "sm",
            icon: "plus",
            onClick: (_e, cx) => app.taxpayerUi.openNewTaxpayer(cx),
          }).mt(SPACE[2]),
        ),
      );
    return SidePanel(cx, "panel-profile", { body });
  }
  const b = app.branch();
  const ui = app.taxpayerUi;
  const body = div().flex().flex_col().gap(2);

  if (ui.panelEdit) {
    const e = ui.panelEdit;
    const label = (text, el, err) => {
      const box = div().flex().flex_col().gap(SPACE[1]).child(Text(cx, "meta", text, "fg2")).child(el);
      if (err) box.child(Text(cx, "meta", err, "danger"));
      return box;
    };
    body.child(
      div()
        .id("panel-edit")
        .flex()
        .flex_col()
        .gap(SPACE[3])
        .pt(SPACE[1])
        .px(SPACE[2])
        .pb(SPACE[2])
        .child(label("Registered name", TextField(cx, e.name, { id: "pe-name", label: "Registered name" })))
        .child(
          label(
            "Branch code",
            b.is_head_office ? Code(cx, b.code, "fg2").font_weight(400) : DigitCells(cx, e.code, "pe-code", { error: Boolean(e.error) }),
            e.error,
          ),
        )
        .child(label("Branch name", TextField(cx, e.branchName, { id: "pe-bname", label: "Branch name" })))
        .child(label("Registered address", TextField(cx, e.address, { id: "pe-address", label: "Registered address" })))
        .child(
          div()
            .flex()
            .flex_row()
            .justify_end()
            .gap(SPACE[2])
            .mt(SPACE[1])
            .child(AppButton(cx, "pe-cancel", "Cancel", { variant: "ghost", size: "sm", onClick: (_e, cx) => ui.cancelPanelEdit(cx) }))
            .child(AppButton(cx, "pe-save", "Save", { variant: "primary", size: "sm", onClick: (_e, cx) => ui.savePanelEdit(cx) })),
        ),
    );
  } else {
    body.child(
      div()
        .relative()
        .flex()
        .flex_col()
        .items_center()
        .gap(SPACE[2])
        .pt(SPACE[3])
        .px(SPACE[2])
        .pb(SPACE[4])
        .child(Avatar(cx, tp.name, 56, tp.tin_root))
        .child(Text(cx, "title", tp.name).text_center().mt(SPACE[1]).whitespace_normal())
        .child(Code(cx, formatTin(tp.tin_root, b.code), "fg2").font_weight(400))
        .child(
          div()
            .absolute()
            .top(SPACE[2])
            .right(0)
            .child(
              IconButton(cx, "panel-edit-btn", "pencil", {
                tooltip: "Edit name, branch and address",
                onClick: (_e, cx) => ui.startPanelEdit(cx),
              }),
            ),
        ),
    );
    const fact = (icon, label, value) =>
      div()
        .flex()
        .flex_row()
        .gap(SPACE[3])
        .child(div().flex().flex_row().items_center().gap(6).w(84).flex_none().child(Icon(cx, icon, { size: SIZE.iconSmall, color: "fg3" })).child(Text(cx, "meta", label, "fg3")))
        .child(div().flex_1().min_w(0).child(typeof value === "string" ? Text(cx, "body", value).whitespace_normal() : value));
    body.child(
      div()
        .flex()
        .flex_col()
        .gap(SPACE[2])
        .mx(SPACE[2])
        .mb(SPACE[1])
        .py(SPACE[3])
        .border_t(1)
        .border_b(1)
        .border_color(t.border)
        .child(fact("git-branch", "Branch", div().flex().flex_row().gap(SPACE[1]).child(Code(cx, b.code)).child(Text(cx, "body", `· ${b.name}`))))
        .child(fact("map-pin", "Address", b.facts.registered_address || "Not set"))
        .child(fact("building", "RDO", b.facts.rdo_code || "Not set")),
    );
  }

  body.child(GroupLabel(cx, "Open"));
  const open = [
    ["profile", "user", "Profile", ""],
    ["forms", "file-text", "Forms", app.yearForms().length],
    ["templates", "layout-template", "Templates", app.templates().length],
  ];
  for (const [view, icon, label, count] of open) {
    body.child(NavRow(cx, `panel-open-${view}`, icon, label, { current: app.view === view, count, onClick: (_e, cx) => app.go(view, cx) }));
  }
  body.child(GroupLabel(cx, tp.branches.length > 1 ? "Branches" : "Branch"));
  for (const br of tp.branches) {
    const current = br.code === b.code;
    body.child(
      NavRow(cx, `panel-branch-${br.code}`, Code(cx, br.code).w(44), br.name, {
        current,
        trailing: current ? Icon(cx, "check", { color: "fg3" }) : null,
        onClick: (_e, cx) => app.selectBranch(br.code, cx),
      }),
    );
  }
  body.child(NavRow(cx, "panel-add-branch", "plus", "Add branch", { onClick: (_e, cx) => ui.openNewBranch(cx) }));
  // Only the admin switches taxpayers; anyone else has just their own.
  if (!app.isAdmin()) return SidePanel(cx, "panel-profile", { body });
  const others = app.taxpayers().filter((x) => x.tin_root !== tp.tin_root);
  if (others.length) body.child(GroupLabel(cx, "Switch taxpayer"));
  for (const x of others) {
    body.child(
      NavRow(cx, `panel-tp-${x.tin_root}`, Avatar(cx, x.name, 20, x.tin_root), x.name, {
        count: x.tin_root.slice(0, 3),
        onClick: (_e, cx) => app.selectTaxpayer(x.tin_root, cx),
      }),
    );
  }
  if (app.canAddTaxpayer()) body.child(NavRow(cx, "panel-new-tp", "plus", "New taxpayer", { onClick: (_e, cx) => ui.openNewTaxpayer(cx) }));
  return SidePanel(cx, "panel-profile", { body });
}

// ---------------------------------------------------------------- palette

export function renderPalette(cx, app) {
  const t = tokens(cx);
  const items = app.paletteItems();
  app.palette.index = Math.min(app.palette.index, Math.max(0, items.length - 1));
  const list = div().id("palette-list").flex().flex_col().gap(2).p(SPACE[3]).max_h(420).overflow_y_scroll().overflow_x_hidden();
  let group = "";
  items.forEach((item, i) => {
    if (item.group !== group) {
      group = item.group;
      // The palette keeps small uppercase group headings (the one exception).
      list.child(div().pt(10).pb(SPACE[1]).px(10).child(Text(cx, "cap", group.toUpperCase(), "fg3").font_weight(600)));
    }
    const on = i === app.palette.index;
    const row = div()
      .id(`palette-item-${i}`)
      .flex()
      .flex_row()
      .items_center()
      .gap(10)
      .min_h(36)
      .py(6)
      .px(10)
      .rounded(RADIUS.row)
      .cursor_pointer()
      .when(on, (el) => el.bg(t.hover))
      .hover((s) => s.bg(t.hover))
      .on_click((_e, cx) => app.runPaletteItem(i, cx))
      .child(item.avatar ? Avatar(cx, item.avatar.name, 20, item.avatar.tin_root) : Icon(cx, item.icon))
      .child(Line(cx, "body", item.label).flex_1());
    if (item.hint) row.child(Text(cx, "meta", item.hint, "fg3"));
    list.child(row);
  });
  if (!items.length) list.child(div().py(40).flex().justify_center().child(Text(cx, "body", "No matches. Try a form code like 2550Q.", "fg3")));
  const card = div()
    .flex()
    .flex_col()
    .on_key_down((e, cx) => {
      if (e.key === "down") {
        app.movePalette(1, cx);
        cx.stop_propagation();
      } else if (e.key === "up") {
        app.movePalette(-1, cx);
        cx.stop_propagation();
      }
    })
    .child(
      div()
        .flex()
        .flex_row()
        .items_center()
        .gap(10)
        .px(SPACE[4])
        .h(48)
        .border_b(1)
        .border_color(t.border)
        .child(Icon(cx, "search", { color: "fg3" }))
        .child(
          TextField(cx, app.palette.input, { id: "palette-input", label: "Search" })
            .border(0)
            .bg(t.surface)
            .px(0)
            .text_size(14)
            .flex_1(),
        )
        .child(Kbd(cx, "Esc")),
    )
    .child(list);
  return Overlay(cx, "palette", card, (cx) => app.closeOverlay(cx), { top: true });
}

// ---------------------------------------------------------------- new return

export function renderNewReturn(cx, app) {
  const t = tokens(cx);
  const tp = app.taxpayer();
  const b = app.branch();
  const year = app.year();
  const forms = app.yearForms();
  const index = app.store.screenIndex();
  const nr = app.newReturn;
  if (!forms.includes(nr.code)) nr.code = forms[0];
  const entry = (code) => index.find((f) => f.c === code) || { c: code, t: code, freq: "" };
  const options = periodOptions(nr.code, entry(nr.code).freq, year);
  if (!options.some((o) => o.value === nr.period)) nr.period = options.length ? options[0].value : 0;
  const tpl = app.templates().find((x) => x.form_code === nr.code);

  const list = div().id("new-return-list").flex().flex_col().gap(2).p(SPACE[3]).max_h(360).overflow_y_scroll().overflow_x_hidden();
  for (const code of forms) {
    const f = entry(code);
    const on = code === nr.code;
    list.child(
      div()
        .id(`new-return-${code}`)
        .flex()
        .flex_row()
        .items_center()
        .gap(10)
        .min_h(36)
        .py(6)
        .px(10)
        .rounded(RADIUS.row)
        .cursor_pointer()
        .when(on, (el) => el.bg(t.selected))
        .hover((s) => (on ? s : s.bg(t.hover)))
        .on_click((_e, cx) => {
          nr.code = code;
          nr.period = null;
          cx.notify();
        })
        .child(Code(cx, code).w(60))
        .child(Line(cx, "body", f.t).flex_1())
        .child(Text(cx, "meta", f.freq, "fg3")),
    );
  }

  const card = div()
    .flex()
    .flex_col()
    .child(
      div()
        .pt(SPACE[4])
        .px(SPACE[5])
        .pb(SPACE[1])
        .child(Text(cx, "title", "Start a return"))
        .child(
          div()
            .flex()
            .flex_row()
            .gap(SPACE[1])
            .mt(2)
            .child(Text(cx, "meta", `${tp.name} ·`, "fg2"))
            .child(Code(cx, formatTin(tp.tin_root, b.code), "fg2").font_weight(400))
            .child(Text(cx, "meta", b.name, "fg2")),
        ),
    )
    .child(list)
    .child(
      div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(SPACE[2])
        .pt(SPACE[3])
        .px(SPACE[5])
        .pb(SPACE[4])
        .border_t(1)
        .border_color(t.border)
        .child(
          div()
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACE[2])
            .child(
              Dropdown(cx, "new-return-period", {
                label: "Period",
                width: 160,
                options,
                value: nr.period,
                open: nr.periodOpen,
                onOpenChange: (open, cx) => {
                  nr.periodOpen = open;
                  cx.notify();
                },
                onChange: (value, cx) => {
                  nr.period = value;
                  cx.notify();
                },
              }),
            )
            .child(tpl ? Chip(cx, "Template", "success", "layout-template") : Chip(cx, "No template")),
        )
        .child(
          div()
            .flex()
            .flex_row()
            .gap(SPACE[2])
            .child(AppButton(cx, "new-return-cancel", "Cancel", { variant: "ghost", onClick: (_e, cx) => app.closeOverlay(cx) }))
            .child(
              AppButton(cx, "new-return-open", "Open form", {
                variant: "primary",
                onClick: (_e, cx) => app.openForm({ tin: b.tin, code: nr.code, year, period: nr.period }, cx),
              }),
            ),
        ),
    );
  return Overlay(cx, "new-return", card, (cx) => app.closeOverlay(cx));
}

