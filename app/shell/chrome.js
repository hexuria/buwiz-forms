// Sidebar + top bar + notifications drawer + footer — the chrome of the app
// shell, ported from bir-desktop/src/sidebar.rs, top_bar.rs and
// components/footer.rs. Layout constants mirror the painted values so the
// frame is visually identical: 280px sidebar (72px mini), 36px top strip,
// 380px notifications drawer, 32px menu rows, 72px footer.

import { div, svg, image } from "gpui-kit";
import { v_flex, h_flex, Input } from "gpui-base";
import { renderNotifications } from "./pages.js";
import { DEV_ORANGE, LOGOUT_RED } from "./theme.js";

const TOP_BAR_HEIGHT = 36;
const DRAWER_WIDTH = 380;
const MENU_ROW_HEIGHT = 32;
const MENU_MINI_SIZE = 36;
const SIDEBAR_W = 280;
const SIDEBAR_MINI_W = 72;

function icon(cx, path, size, color) {
  return svg(path).size(size).text_color(color || cx.theme().colors.foreground);
}

/** 36px header strip: dev pills area left, bell + theme buttons right. */
export function renderTopBar(cx, state) {
  const badge =
    state.notificationCount >= 10
      ? "9+"
      : state.notificationCount > 0
        ? String(state.notificationCount)
        : null;

  const bell = div()
    .id("nav_notifications")
    .relative()
    .flex()
    .items_center()
    .justify_center()
    .size(28)
    .rounded_md()
    .when(state.notificationsOpen, (el) => el.bg(cx.theme().colors.accent))
    .hover((s) => s.bg(cx.theme().colors.muted))
    .on_click((_ev, cx) => state.toggleDrawer(cx))
    .child(icon(cx, "icons/bell.svg", 18));

  if (badge) {
    bell.child(
      div()
        .absolute()
        .top(-2)
        .right(-4)
        .min_w(16)
        .h(16)
        .px(3)
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(LOGOUT_RED)
        .text_color("#ffffff")
        .text_size(10)
        .font_semibold()
        .child(badge),
    );
  }

  const themeBtn = div()
    .id("theme_toggle_btn")
    .relative()
    .flex()
    .items_center()
    .justify_center()
    .size(28)
    .rounded_md()
    .hover((s) => s.bg(cx.theme().colors.muted))
    .on_click((_ev, cx) => state.cycleTheme(cx))
    .child(icon(cx, cx.theme().is_dark ? "icons/moon.svg" : "icons/sun.svg", 18));

  return div()
    .id("top_bar")
    .flex()
    .flex_row()
    .flex_none()
    .items_center()
    .w_full()
    .h(TOP_BAR_HEIGHT)
    .pr_2()
    .border_b(1)
    .border_color(cx.theme().colors.border)
    .bg(cx.theme().colors.background)
    .child(div().flex_1().min_w_0().overflow_hidden())
    .child(
      div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(4)
        .child(bell)
        .child(themeBtn),
    );
}

/** Right-edge drawer over the content area (notifications). */
export function renderNotificationsDrawer(cx, state) {
  if (!state.notificationsOpen) return div();
  return div()
    .id("page_notifications")
    .absolute()
    .top(TOP_BAR_HEIGHT)
    .right(0)
    .bottom(0)
    .w(DRAWER_WIDTH)
    .max_w_full()
    .flex()
    .flex_col()
    .bg(cx.theme().colors.background)
    .border_l(1)
    .border_color(cx.theme().colors.border)
    .shadow_lg()
    .child(renderNotifications(cx, state));
}

function menuRow(cx, state, { id, iconPath, label, view }) {
  const isMini = state.isMiniSidebar;
  const active = state.view === view;
  const row = div()
    .id(id)
    .flex()
    .items_center()
    .w_full()
    .hover((s) => s.bg(cx.theme().colors.muted))
    .when(isMini, (el) =>
      el.justify_center().size(MENU_MINI_SIZE).flex_none().rounded_full().bg(cx.theme().colors.secondary),
    )
    .when(!isMini, (el) =>
      el
        .justify_start()
        .h(MENU_ROW_HEIGHT)
        .px(12)
        .gap(12)
        .rounded_md()
        .bg(cx.theme().colors.secondary),
    )
    .when(active, (el) => el.bg(cx.theme().colors.accent))
    .on_click((_ev, cx) => state.navigate(view, cx))
    .child(icon(cx, iconPath, 20));
  if (!isMini) {
    row.child(
      div().text_size(14).text_color(cx.theme().colors.foreground).child(label),
    );
  }
  return row;
}

/** Orange "Dev" chip beside Settings — debug builds only in the painted app. */
function devModeChip(cx, state) {
  const isMini = state.isMiniSidebar;
  const active = state.view === "dev_mode";
  const chip = div()
    .id("dev_mode_sidebar_btn")
    .flex()
    .flex_none()
    .items_center()
    .justify_center()
    .gap(4)
    .h(isMini ? 24 : MENU_ROW_HEIGHT)
    .px(8)
    .rounded_full()
    .border(1)
    .border_color(DEV_ORANGE)
    .when(active, (el) => el.bg("#EA580C40"))
    .hover((s) => s.bg("#EA580C26"))
    .on_click((_ev, cx) => state.navigate("dev_mode", cx))
    .child(
      div()
        .text_size(12)
        .font_semibold()
        .text_color(DEV_ORANGE)
        .child("Dev"),
    );
  if (!isMini) {
    chip.child(icon(cx, "icons/square-terminal.svg", 14, DEV_ORANGE));
  }
  return chip;
}

function profileRow(cx, state, profile) {
  const isMini = state.isMiniSidebar;
  const active = state.activeProfileTin === profile.tin;
  const row = div()
    .id(`profile-${profile.id}`)
    .w_full()
    .when(!isMini, (el) => el.py(8).px(24))
    .when(isMini, (el) => el.py(8).px(12))
    .bg(active ? cx.theme().colors.accent : "#00000000")
    .border(1)
    .border_color(active ? cx.theme().colors.border : "#00000000")
    .flex()
    .flex_col()
    .hover((s) => (active ? s : s.bg(cx.theme().colors.muted)))
    .on_click((_ev, cx) => state.selectProfile(profile, cx));

  if (isMini) {
    const initials = (profile.full_name || "?")
      .split(/\s+/)
      .map((w) => w[0])
      .filter(Boolean)
      .slice(0, 2)
      .join("")
      .toUpperCase();
    return row.child(
      div()
        .size(48)
        .flex_none()
        .rounded_full()
        .bg(cx.theme().colors.secondary)
        .flex()
        .items_center()
        .justify_center()
        .child(
          div()
            .text_size(14)
            .font_semibold()
            .text_color(cx.theme().colors.primary)
            .child(initials),
        ),
    );
  }

  return row.child(
    v_flex()
      .child(
        div()
          .font_semibold()
          .text_color(
            profile.is_archived
              ? cx.theme().colors.muted_foreground
              : cx.theme().colors.foreground,
          )
          .child(
            profile.is_archived
              ? `${profile.full_name} (Archived)`
              : profile.full_name || "(unnamed)",
          ),
      )
      .child(
        div()
          .text_size(14)
          .text_color(cx.theme().colors.muted_foreground)
          .child(`TIN: ${profile.tin}`),
      ),
  );
}

/** Full sidebar: logo, profiles list, search, archived toggle, menu rows. */
export function renderSidebar(cx, state) {
  const isMini = state.isMiniSidebar;
  const filter = (state.profileFilter || "").trim().toLowerCase();
  const archivedCount = state.profiles.filter((p) => p.is_archived).length;
  const visible = state.profiles.filter((p) => {
    if (state.showArchived !== p.is_archived) return false;
    if (!filter) return true;
    return (
      (p.full_name || "").toLowerCase().includes(filter) ||
      (p.tin || "").toLowerCase().includes(filter)
    );
  });

  const toggleIcon = isMini ? "icons/chevron-right.svg" : "icons/chevron-left.svg";

  const headerRow = h_flex()
    .w_full()
    .when(!isMini, (el) => el.px(24))
    .when(isMini, (el) => el.px(12))
    .items_center()
    .justify_between()
    .when(!isMini, (el) =>
      el.child(
        div()
          .text_size(12)
          .font_semibold()
          .text_color(cx.theme().colors.muted_foreground)
          .child("TAXPAYER PROFILES"),
      ),
    )
    .when(isMini, (el) => el.justify_center())
    .child(
      div()
        .id("add_profile_mini_btn")
        .flex()
        .items_center()
        .justify_center()
        .size(40)
        .flex_none()
        .hover((s) => s.bg(cx.theme().colors.muted).rounded_md())
        .on_click((_ev, cx) => state.openNewProfile(cx))
        .child(
          div()
            .text_size(20)
            .font_semibold()
            .text_color(cx.theme().colors.foreground)
            .child("+"),
        ),
    );

  const searchArea = isMini
    ? div()
        .px(12)
        .flex()
        .justify_center()
        .w_full()
        .child(
          div()
            .id("sidebar_search_mini_btn")
            .flex()
            .items_center()
            .justify_center()
            .size(48)
            .rounded_full()
            .bg(cx.theme().colors.secondary)
            .flex_none()
            .hover((s) => s.bg(cx.theme().colors.muted))
            .on_click((_ev, cx) => state.expandSidebarAndFocusSearch(cx))
            .child(icon(cx, "icons/search.svg", 20)),
        )
    : div().px(24).w_full().child(
        Input.new(state.searchInput)
          .w_full()
          .h(28)
          .px(8)
          .border(1)
          .border_color(cx.theme().colors.input)
          .bg(cx.theme().colors.surface)
          .text_size(12),
      );

  const logout = state.activeSessionTin
    ? div()
        .id("logout_btn_full")
        .flex()
        .items_center()
        .gap(12)
        .mx(24)
        .px(12)
        .h(MENU_ROW_HEIGHT)
        .rounded_md()
        .bg("#EF444421")
        .hover((s) => s.bg("#EF444440"))
        .on_click((_ev, cx) => state.logout(cx))
        .child(icon(cx, "icons/power.svg", 18, LOGOUT_RED))
        .child(
          div()
            .text_size(14)
            .font_semibold()
            .text_color(LOGOUT_RED)
            .child("Logout"),
        )
    : div();

  const archivedToggle =
    archivedCount > 0 || state.showArchived
      ? div()
          .w_full()
          .py(8)
          .when(!isMini, (el) => el.px(24))
          .when(isMini, (el) => el.px(12))
          .flex()
          .justify_center()
          .child(
            isMini
              ? div()
                  .id("toggle_archived_mini_btn")
                  .flex()
                  .items_center()
                  .justify_center()
                  .size(48)
                  .flex_none()
                  .rounded_full()
                  .bg(cx.theme().colors.secondary)
                  .hover((s) => s.bg(cx.theme().colors.muted))
                  .on_click((_ev, cx) => state.toggleArchived(cx))
                  .child(
                    icon(
                      cx,
                      state.showArchived ? "icons/eye-off.svg" : "icons/eye.svg",
                      20,
                    ),
                  )
              : div()
                  .id("toggle_archived")
                  .flex()
                  .items_center()
                  .justify_center()
                  .h(MENU_ROW_HEIGHT)
                  .px(12)
                  .rounded_md()
                  .bg(cx.theme().colors.secondary)
                  .text_size(13)
                  .text_color(cx.theme().colors.secondary_foreground)
                  .hover((s) => s.bg(cx.theme().colors.muted))
                  .on_click((_ev, cx) => state.toggleArchived(cx))
                  .child(
                    state.showArchived
                      ? "Hide Archived Profiles"
                      : `Show ${archivedCount} Archived`,
                  ),
          )
      : div();

  return div()
    .id("sidebar")
    .w(isMini ? SIDEBAR_MINI_W : SIDEBAR_W)
    .h_full()
    .bg(cx.theme().colors.background)
    .border_r(1)
    .border_color(cx.theme().colors.border)
    .when(!isMini, (el) => el.py(16))
    .when(isMini, (el) => el.py(12))
    .flex()
    .flex_col()
    .justify_between()
    .child(
      v_flex()
        .gap(8)
        .flex_1()
        .overflow_hidden()
        .child(
          v_flex()
            .w_full()
            .when(!isMini, (el) => el.px(24))
            .when(isMini, (el) => el.px(12))
            .child(
              h_flex()
                .w_full()
                .when(!isMini, (el) => el.justify_end())
                .when(isMini, (el) => el.justify_center())
                .mb(4)
                .child(
                  div()
                    .id("sidebar_toggle_btn")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(40)
                    .flex_none()
                    .hover((s) => s.bg(cx.theme().colors.muted).rounded_md())
                    .on_click((_ev, cx) => state.toggleMiniSidebar(cx))
                    .child(icon(cx, toggleIcon, 28)),
                ),
            )
            .child(
              div()
                .id("global_dashboard_btn")
                .w_full()
                .on_click((_ev, cx) => state.navigate("global_dashboard", cx))
                .child(
                  isMini
                    ? svg("icons/e_logo.svg").w_full().h(32)
                    : image("images/ebirforms.png").w_full().h(40),
                ),
            ),
        )
        .child(headerRow)
        .child(searchArea)
        .child(div().w_full().h(1).bg(cx.theme().colors.border))
        .child(
          v_flex()
            .id("sidebar_profile_list")
            .flex_1()
            .overflow_y_scroll()
            .pt(8)
            .gap(8)
            .children(visible.map((p) => profileRow(cx, state, p)))
            .child(archivedToggle),
        )
        .child(logout)
        .child(div().w_full().h(1).bg(cx.theme().colors.border)),
    )
    .child(
      v_flex()
        .gap(4)
        .w_full()
        .when(!isMini, (el) => el.px(24))
        .when(isMini, (el) => el.px(12))
        .child(
          menuRow(cx, state, {
            id: "import_export_sidebar_btn",
            iconPath: "icons/hard-drive.svg",
            label: "Import Data",
            view: "import_export",
          }),
        )
        .child(
          menuRow(cx, state, {
            id: "cron_tasks_sidebar_btn",
            iconPath: "icons/square-terminal.svg",
            label: "Background Tasks",
            view: "cron_tasks",
          }),
        )
        .child(
          menuRow(cx, state, {
            id: "admin_calendar_sidebar_btn",
            iconPath: "icons/calendar.svg",
            label: "Tax Calendars",
            view: "admin_calendar",
          }),
        )
        .child(
          h_flex()
            .w_full()
            .gap(4)
            .when(isMini, (el) => el.flex_col().items_center())
            .when(!isMini, (el) => el.items_center())
            .child(
              div()
                .when(!isMini, (el) => el.flex_1())
                .child(
                  menuRow(cx, state, {
                    id: "settings_sidebar_btn",
                    iconPath: "icons/settings.svg",
                    label: "Settings",
                    view: "settings",
                  }),
                ),
            )
            .child(devModeChip(cx, state)),
        ),
    );
}

/** Footer strip: Bagong Pilipinas + BIR seals left, copyright + logo right. */
export function renderFooter(cx) {
  return div()
    .w_full()
    .min_h(72)
    .px(16)
    .py(8)
    .flex()
    .flex_row()
    .flex_wrap()
    .gap(8)
    .items_center()
    .justify_between()
    .bg(cx.theme().colors.background)
    .border_t(1)
    .border_color(cx.theme().colors.border)
    .child(
      h_flex()
        .flex_none()
        .items_center()
        .gap(16)
        .child(svg("icons/bagong-pilipinas.svg").w(48).h(48))
        .child(image("images/bir-new-logo.png").w(48).h(48)),
    )
    .child(
      h_flex()
        .items_center()
        .gap(8)
        .min_w_0()
        .max_w_full()
        .child(
          div()
            .flex_1()
            .min_w_0()
            .text_size(12)
            .text_color(cx.theme().colors.muted_foreground)
            .child("© 2026 Goldcoders Corp. goldcoders.dev"),
        )
        .child(
          div()
            .id("goldcoders_logo")
            .flex_none()
            .on_click((_ev, cx2) => cx2.open_url("https://www.facebook.com/goldcoders.corp"))
            .child(
              image("images/goldcoders_logo.png").w(48).h(48),
            ),
        ),
    );
}
