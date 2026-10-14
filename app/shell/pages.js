// Page bodies for the app shell. S1 scope: real layouts wired to navigation,
// using live bir-core data where the painted page needs it; deep view bodies
// (profile editor, settings tabs, form views, queue) land in later slices.
//
// Layout grammar mirrors bir-desktop: page roots are vertical flex, content
// scrolls, rows are h_flex with px/py rhythm on the 4px grid.

import { div } from "gpui-kit";
import { v_flex, h_flex } from "gpui-base";

const FORM_CODE_W = 88;

function muted(cx) {
  return cx.theme().colors.muted_foreground;
}

export function pageShell(children, cx) {
  return v_flex()
    .size_full()
    .min_w_0()
    .bg(cx.theme().colors.background)
    .children(children);
}

export function pageHeading(text, cx) {
  return div()
    .text_size(20)
    .font_semibold()
    .text_color(cx.theme().colors.foreground)
    .child(text);
}

export function pageSub(text, cx) {
  return div()
    .text_size(13)
    .text_color(muted(cx))
    .child(text);
}

export function stubNotice(cx, name, slice) {
  return div()
    .px(12)
    .py(10)
    .rounded_md()
    .bg(cx.theme().colors.muted)
    .child(
      div()
        .text_size(12)
        .text_color(muted(cx))
        .child(`${name} is stubbed for the app-shell slice — full port lands in ${slice}.`),
    );
}

const CATEGORY_ORDER = [
  "Withholding Tax",
  "Income Tax",
  "Value-Added Tax",
  "Percentage Tax",
  "Excise Tax",
  "Documentary Stamp Tax",
  "Other",
];

function categoryRank(category) {
  const i = CATEGORY_ORDER.indexOf(category);
  return i < 0 ? CATEGORY_ORDER.length : i;
}

function groupByCategory(forms) {
  const groups = new Map();
  for (const f of forms) {
    const key = f.category || "Other";
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(f);
  }
  return [...groups.entries()].sort(
    (a, b) => categoryRank(a[0]) - categoryRank(b[0]) || a[0].localeCompare(b[0]),
  );
}

function formRow(cx, form, onOpen) {
  return h_flex()
    .id(`form-${form.code}`)
    .w_full()
    .px(12)
    .py(8)
    .gap(10)
    .items_center()
    .rounded_md()
    .hover((s) => s.bg(cx.theme().colors.muted))
    .on_click((_ev, cx) => onOpen(form, cx))
    .child(
      div()
        .w(FORM_CODE_W)
        .flex_none()
        .text_size(13)
        .font_semibold()
        .text_color(cx.theme().colors.primary)
        .child(form.code),
    )
    .child(
      v_flex()
        .flex_1()
        .min_w_0()
        .child(
          div()
            .text_size(13)
            .text_color(cx.theme().colors.foreground)
            .child(form.title),
        )
        .child(
          div()
            .text_size(11)
            .text_color(muted(cx))
            .child(`${form.frequency}${form.requires_employees ? " · payroll" : ""}`),
        ),
    );
}

/**
 * Global dashboard — no taxpayer selected. The painted page is a landing
 * surface; until its full port lands (S3), it hosts the whole BIR form
 * catalog grouped by category, which is live `FORM_REGISTRY` data.
 */
export function renderGlobalDashboard(cx, state) {
  const forms = (state.forms || []).filter((f) => !f.is_deprecated);
  const groups = groupByCategory(forms);
  return pageShell(
    [
      v_flex()
        .px(20)
        .py(16)
        .gap(4)
        .child(pageHeading("Tax Forms", cx))
        .child(
          pageSub(
            `${forms.length} forms in the BIR catalog · pick a taxpayer profile to start a return`,
            cx,
          ),
        ),
      v_flex()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .px(16)
        .pb(16)
        .gap(12)
        .children(
          groups.map(([category, members]) =>
            v_flex()
              .gap(2)
              .child(
                div()
                  .px(12)
                  .pt(6)
                  .pb(2)
                  .text_size(11)
                  .font_semibold()
                  .text_color(muted(cx))
                  .child(`${category.toUpperCase()} · ${members.length}`),
              )
              .children(
                members.map((form) =>
                  formRow(cx, form, (f, clickCx) => state.openFormCatalogEntry(f, clickCx)),
                ),
              ),
          ),
        ),
    ],
    cx,
  );
}

/**
 * Per-profile dashboard: the taxpayer's filing library — every catalog form
 * the profile's registrations make applicable. Parity anchor for later form
 * slices; drafting progress/cards land with S3.
 */
export function renderDashboard(cx, state, profile) {
  const applicable = new Set(profile.applicable_forms || []);
  const forms = (state.forms || []).filter(
    (f) => !f.is_deprecated && (applicable.size === 0 || applicable.has(f.code)),
  );
  const groups = groupByCategory(forms);
  return pageShell(
    [
      v_flex()
        .px(20)
        .py(16)
        .gap(4)
        .child(
          h_flex()
            .items_center()
            .gap(12)
            .child(pageHeading(profile.full_name || "Taxpayer", cx))
            .child(
              div()
                .px(8)
                .py(2)
                .rounded_full()
                .bg(cx.theme().colors.secondary)
                .text_size(11)
                .text_color(cx.theme().colors.secondary_foreground)
                .child(`TIN ${profile.tin}`),
            ),
        )
        .child(
          pageSub(
            `${profile.taxpayer_type} · RDO ${profile.rdo_code} · ${forms.length} applicable forms`,
            cx,
          ),
        ),
      v_flex()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .px(16)
        .pb(16)
        .gap(12)
        .children(
          [
            ...(() => {
              const drafts = state.draftsForProfile
                ? state.draftsForProfile(new Date().getFullYear())
                : [];
              if (drafts.length === 0) return [];
              return [
                v_flex()
                  .gap(2)
                  .child(
                    div()
                      .px(12)
                      .pt(6)
                      .pb(2)
                      .text_size(11)
                      .font_semibold()
                      .text_color(muted(cx))
                      .child(`IN PROGRESS · ${drafts.length}`),
                  )
                  .children(
                    drafts.map((draft) =>
                      h_flex()
                        .w_full()
                        .px(12)
                        .py(8)
                        .gap(10)
                        .items_center()
                        .rounded_md()
                        .hover((s) => s.bg(cx.theme().colors.muted))
                        .on_click((_ev, clickCx) =>
                          state.openDraftSummary(draft, clickCx),
                        )
                        .child(
                          div()
                            .w(FORM_CODE_W)
                            .flex_none()
                            .text_size(13)
                            .font_semibold()
                            .text_color(cx.theme().colors.primary)
                            .child(draft.form_code),
                        )
                        .child(
                          div()
                            .flex_1()
                            .min_w_0()
                            .text_size(13)
                            .text_color(cx.theme().colors.foreground)
                            .child(
                              `${draft.taxable_year}${draft.quarter ? ` Q${draft.quarter}` : ""}${draft.month ? ` M${draft.month}` : ""} · ${draft.status} · updated ${draft.updated_at}`,
                            ),
                        ),
                    ),
                  ),
              ];
            })(),
            ...groups.map(([category, members]) =>
            v_flex()
              .gap(2)
              .child(
                div()
                  .px(12)
                  .pt(6)
                  .pb(2)
                  .text_size(11)
                  .font_semibold()
                  .text_color(muted(cx))
                  .child(`${category.toUpperCase()} · ${members.length}`),
              )
              .children(
                members.map((form) =>
                  formRow(cx, form, (f, clickCx) => state.openFormCatalogEntry(f, clickCx)),
                ),
              ),
          ),
          ],
        ),
    ],
    cx,
  );
}

/** Stub page for sections whose real bodies port in later slices. */
export function renderStub(cx, title, subtitle, slice) {
  return pageShell(
    [
      v_flex()
        .px(20)
        .py(16)
        .gap(4)
        .child(pageHeading(title, cx))
        .child(pageSub(subtitle, cx)),
      div().px(16).child(stubNotice(cx, title, slice)),
    ],
    cx,
  );
}

/**
 * Notifications list body inside the drawer. Painted app pulls these from the
 * notification store; until the store port lands (S2) it is an honest empty
 * state — the badge stays hidden at zero either way.
 */
export function renderNotifications(cx) {
  return v_flex()
    .size_full()
    .px(16)
    .py(16)
    .gap(8)
    .child(pageHeading("Notifications", cx))
    .child(
      v_flex()
        .py(24)
        .w_full()
        .items_center()
        .child(div().text_size(13).text_color(muted(cx)).child("No notifications")),
    );
}
