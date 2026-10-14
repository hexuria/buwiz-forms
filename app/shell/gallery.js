// Components gallery: every wrapper from ui.js in its states, so they can be
// compared with the prototype's Components tab in both themes. Reached from
// the command palette ("Components").

import { div } from "gpui-kit";
import {
  tokens,
  PALETTES,
  SPACE,
  RADIUS,
  Text,
  Icon,
  IconButton,
  AppButton,
  NavRow,
  ContextPill,
  Avatar,
  Disclosure,
  DisclosureGroup,
  Field,
  FieldGrid,
  StatusChip,
  Chip,
  SidePanel,
  Segmented,
  Switch,
  Checkbox,
  PageHeader,
  SectionTitle,
} from "./ui.js";

function demo(cx, id, title, children, col) {
  const t = tokens(cx);
  const box = div()
    .id(id)
    .flex()
    .flex_wrap()
    .items_center()
    .gap(SPACE[3])
    .p(SPACE[5])
    .min_h(76)
    .bg(t.bg)
    .when(col, (el) => el.flex_col().items_start())
    .when(!col, (el) => el.flex_row());
  for (const c of children) box.child(c);
  return div()
    .flex()
    .flex_col()
    .mt(SPACE[4])
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .overflow_hidden()
    .child(box)
    .child(div().py(SPACE[3]).px(SPACE[5]).border_t(1).border_color(t.border).child(Text(cx, "emph", title)));
}

export function renderGallery(cx, app, width) {
  const g = app.gallery || (app.gallery = { seg: "Monthly", sw: true, chk: true, open: true });
  const noop = (_e, cx) => cx.notify();
  const palette = cx.theme().is_dark ? PALETTES.dark : PALETTES.light;
  const swatches = div().flex().flex_row().flex_wrap().gap(SPACE[3]);
  for (const [name, value] of Object.entries(palette)) {
    if (/knob|caption/.test(name)) continue;
    swatches.child(
      div()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACE[3])
        .w(200)
        .child(div().size(40).rounded(RADIUS.row).border(1).border_color(tokens(cx).border).bg(value))
        .child(div().flex().flex_col().child(Text(cx, "control", name)).child(Text(cx, "meta", value, "fg2"))),
    );
  }
  return div()
    .flex()
    .flex_col()
    .child(PageHeader(cx, { title: "Components", lede: "Every wrapper in app/shell/ui.js, at its fixed sizes. Switch the theme to compare light and dark." }))
    .child(div().mt(28).child(SectionTitle(cx, "Color", true)))
    .child(swatches)
    .child(SectionTitle(cx, "Wrappers"))
    .child(
      demo(cx, "gallery-iconbutton", "IconButton · 28 × 28, icon 16, tooltip required", [
        IconButton(cx, "g-ib-1", "panel-left", { tooltip: "Hide sidebar", onClick: noop }),
        IconButton(cx, "g-ib-2", "bell", { tooltip: "Notifications", dot: true, onClick: noop }),
        IconButton(cx, "g-ib-3", "bell", { tooltip: "Notifications (open)", pressed: true, onClick: noop }),
        IconButton(cx, "g-ib-4", "arrow-left", { tooltip: "Back", disabled: true }),
      ]),
    )
    .child(
      demo(cx, "gallery-appbutton", "AppButton · 32 (md) and 28 (sm), label 13/500, icon 16, gap 6", [
        AppButton(cx, "g-b-1", "Save changes", { variant: "primary", onClick: noop }),
        AppButton(cx, "g-b-2", "Copy TIN", { icon: "copy", onClick: noop }),
        AppButton(cx, "g-b-3", "Cancel", { variant: "ghost", onClick: noop }),
        AppButton(cx, "g-b-4", "Delete", { variant: "danger", onClick: noop }),
        AppButton(cx, "g-b-5", "New return", { variant: "primary", size: "sm", icon: "plus", onClick: noop }),
        AppButton(cx, "g-b-6", "Edit", { variant: "ghost", size: "sm", icon: "pencil", onClick: noop }),
      ]),
    )
    .child(
      demo(
        cx,
        "gallery-navrow",
        "NavRow · 32 tall, gap 10, radius 8",
        [
          div()
            .w(232)
            .flex()
            .flex_col()
            .gap(2)
            .child(NavRow(cx, "g-n-1", "house", "Home", { current: true, onClick: noop }))
            .child(NavRow(cx, "g-n-2", "file-text", "Forms", { count: 7, onClick: noop }))
            .child(NavRow(cx, "g-n-3", "lock", "Lock app", { kbd: "Ctrl L", onClick: noop })),
        ],
        true,
      ),
    )
    .child(
      demo(cx, "gallery-pill", "ContextPill · 30 tall, avatar 20, chevron 14", [
        ContextPill(cx, "g-p-1", { name: "Example Trading Corp", hueKey: "123456788", tin: "123-456-788-00000", onClick: noop }),
        ContextPill(cx, "g-p-2", { name: "Example Trading Corp", hueKey: "123456788", tin: "123-456-788-00000", open: true, onClick: noop }),
        ContextPill(cx, "g-p-3", { name: "Example Trading Corp", hueKey: "123456788", branchCode: "00000", compact: true, onClick: noop }),
        ContextPill(cx, "g-p-4", { empty: true, onClick: noop }),
      ]),
    )
    .child(
      demo(cx, "gallery-avatar", "Avatar · 20, 28, 40, 56", [
        Avatar(cx, "Example Trading Corp", 20, "a"),
        Avatar(cx, "Example Trading Corp", 28, "a"),
        Avatar(cx, "Sample Store", 40, "b"),
        Avatar(cx, "Demo Holdings Inc", 56, "c"),
      ]),
    )
    .child(
      demo(cx, "gallery-chips", "StatusChip · 20 tall, 11/500, soft tint, icon 12", [
        StatusChip(cx, "not_started"),
        StatusChip(cx, "draft"),
        StatusChip(cx, "filed"),
        StatusChip(cx, "due_soon", 6),
        StatusChip(cx, "overdue", -3),
        Chip(cx, "From tax profile", "neutral", "user"),
      ]),
    )
    .child(
      demo(cx, "gallery-segmented", "Segmented · 2 px track, 24 px segments, 12/500 · Switch · Checkbox", [
        Segmented(
          cx,
          "g-seg",
          ["All", "Monthly", "Quarterly", "Annual"].map((x) => ({ value: x, label: x })),
          g.seg,
          (v, cx) => {
            g.seg = v;
            cx.notify();
          },
        ),
        Switch(cx, "g-sw", g.sw, (v, cx) => {
          g.sw = v;
          cx.notify();
        }),
        Checkbox(cx, "g-chk", g.chk, (v, cx) => {
          g.chk = v;
          cx.notify();
        }),
        Icon(cx, "check"),
      ]),
    )
    .child(
      demo(
        cx,
        "gallery-disclosure",
        "Disclosure · one bordered group, 44 px headers, body 4 / 20 / 20 / 36 · Field",
        [
          div()
            .w(Math.min(560, width - 42))
            .child(
              DisclosureGroup(cx, "g-disc", [
                Disclosure(cx, "g-disc-1", {
                  title: "Taxpayer",
                  meta: "Shared by every branch",
                  open: g.open,
                  onToggle: (_e, cx) => {
                    g.open = !g.open;
                    cx.notify();
                  },
                  onEdit: noop,
                  body: FieldGrid(cx, Math.min(560, width - 42) - 58, [
                    { full: true, el: Field(cx, "Registered name", { value: "Example Trading Corp" }) },
                    { el: Field(cx, "TIN", { value: "123-456-788" }) },
                    { el: Field(cx, "ZIP code", { value: "1226", inherited: true }) },
                  ]),
                }),
                Disclosure(cx, "g-disc-2", { title: "Branch", meta: "Head office", open: false, onToggle: noop, body: div() }),
              ]),
            ),
        ],
        true,
      ),
    )
    .child(
      demo(cx, "gallery-sidepanel", "SidePanel · 320 wide, 40 px header, at most one ghost action, no close button", [
        div()
          .h(200)
          .rounded(RADIUS.card)
          .border(1)
          .border_color(tokens(cx).border)
          .overflow_hidden()
          .child(
            SidePanel(cx, "g-panel", {
              title: "Notifications",
              action: AppButton(cx, "g-panel-action", "Mark all read", { variant: "ghost", size: "sm", onClick: noop }),
              body: Text(cx, "body", "No notifications yet.", "fg3"),
            }),
          ),
      ]),
    );
}
