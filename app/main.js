// buwiz-forms on gpui-shell — S0 skeleton.
//
// The navigation chrome and data plumbing are real (the `bir` host module
// reads the live SQLCipher database through bir-core, same owner-lock rules as
// painted `bir`); the visual port happens in slices S1+ per
// docs/plans/gpui-shell-migration.md / issue #135.

import { View, div } from "gpui-kit";
import { v_flex, h_flex } from "gpui-base";
import { TitleBar } from "gpui-component";
/** @import { Context } from "gpui-kit */
import {
  system_info,
  list_profiles,
  forms_catalog,
} from "bir";

const SPACE = { xxs: 2, xs: 4, sm: 8, md: 12, lg: 16, xl: 24 };

/** @param {import("gpui-kit").Context} cx */
const muted = (value, cx) =>
  div().text_size(12).text_color(cx.theme().colors.muted_foreground).child(value);

/** @param {import("gpui-kit").Context} cx */
const heading = (value, cx) =>
  div().text_size(13).font_semibold().text_color(cx.theme().colors.foreground).child(value);

export default class BirShell extends View {
  /** @param {unknown} _props @param {import("gpui-kit").AsyncContext} _cx */
  init(_props, _cx) {
    /** @type {import("bir").SystemInfo} */
    this.info = system_info();
    /** @type {import("bir").ProfileSummary[]} */
    this.profiles = [];
    /** @type {string | null} */
    this.profilesError = null;
    try {
      this.profiles = list_profiles();
    } catch (error) {
      this.profilesError = `${error}`;
    }
    /** @type {import("bir").FormCatalogEntry[]} */
    this.forms = forms_catalog().filter((form) => !form.is_deprecated);
    /** @type {string} */
    this.category = this.categories()[0] ?? "all";
    /** @type {number | null} */
    this.selectedProfile = this.profiles[0]?.id ?? null;
  }

  categories() {
    const seen = new Set(this.forms.map((form) => form.category));
    return [...seen].sort();
  }

  visibleForms() {
    return this.forms.filter((form) => form.category === this.category);
  }

  /** @param {number} id @param {Context} cx */
  selectProfile(id, cx) {
    this.selectedProfile = id;
    cx.notify();
  }

  /** @param {string} category @param {Context} cx */
  selectCategory(category, cx) {
    this.category = category;
    cx.notify();
  }

  /** @param {import("bir").ProfileSummary} profile @param {Context} cx */
  profileRow(profile, cx) {
    const active = profile.id === this.selectedProfile;
    return v_flex()
      .id(`profile-${profile.id}`)
      .w_full()
      .px(SPACE.md)
      .py(SPACE.sm)
      .gap(SPACE.xxs)
      .bg(active ? cx.theme().colors.muted : cx.theme().colors.surface)
      .hover((style) => style.bg(cx.theme().colors.muted))
      .on_click((_event, cx) => this.selectProfile(profile.id, cx))
      .child(heading(profile.full_name || profile.tin, cx))
      .child(
        muted(`TIN ${profile.tin} · RDO ${profile.rdo_code} · ${profile.taxpayer_type}`, cx),
      );
  }

  /** @param {string} category @param {Context} cx */
  categoryRow(category, cx) {
    const active = category === this.category;
    return div()
      .id(`cat-${category}`)
      .w_full()
      .px(SPACE.md)
      .py(SPACE.xs)
      .text_size(12)
      .text_color(
        active
          ? cx.theme().colors.foreground
          : cx.theme().colors.muted_foreground,
      )
      .hover((style) => style.bg(cx.theme().colors.muted))
      .on_click((_event, cx) => this.selectCategory(category, cx))
      .child(category);
  }

  /** @param {import("bir").FormCatalogEntry} form @param {Context} cx */
  formRow(form, cx) {
    return h_flex()
      .w_full()
      .px(SPACE.lg)
      .py(SPACE.sm)
      .gap(SPACE.lg)
      .items_baseline()
      .child(
        div()
          .w(72)
          .flex_none()
          .text_size(13)
          .font_medium()
          .text_color(cx.theme().colors.foreground)
          .child(form.code),
      )
      .child(
        v_flex()
          .flex_1()
          .gap(SPACE.xxs)
          .child(heading(form.title, cx))
          .child(muted(form.frequency, cx)),
      );
  }

  /** @param {Context} cx */
  sidebar(cx) {
    const profiles = this.profilesError
      ? muted(`database: ${this.profilesError}`, cx)
      : this.profiles.length === 0
        ? muted("No taxpayer profiles yet", cx)
        : v_flex().children(this.profiles.map((p) => this.profileRow(p, cx)));

    return v_flex()
      .w(280)
      .flex_none()
      .h_full()
      .bg(cx.theme().colors.surface)
      .border_r(1)
      .border_color(cx.theme().colors.border)
      .child(
        v_flex()
          .px(SPACE.lg)
          .py(SPACE.md)
          .gap(SPACE.xs)
          .child(heading("Profiles", cx))
          .child(muted(`${this.profiles.length} registered`, cx)),
      )
      .child(div().h(1).w_full().bg(cx.theme().colors.border))
      .child(v_flex().py(SPACE.sm).child(profiles))
      .child(div().h(1).w_full().bg(cx.theme().colors.border))
      .child(
        v_flex()
          .px(SPACE.lg)
          .py(SPACE.md)
          .gap(SPACE.xs)
          .child(heading("Forms", cx))
          .child(muted(`${this.forms.length} in catalog`, cx)),
      )
      .child(
        v_flex()
          .flex_1()
          .min_h_0()
          .overflow_y_scroll()
          .pb(SPACE.md)
          .children(this.categories().map((category) => this.categoryRow(category, cx))),
      );
  }

  /** @param {Context} cx */
  content(cx) {
    const forms = this.visibleForms();
    return v_flex()
      .flex_1()
      .min_w_0()
      .h_full()
      .child(
        h_flex()
          .w_full()
          .px(SPACE.xl)
          .py(SPACE.md)
          .items_center()
          .justify_between()
          .child(heading(this.category, cx))
          .child(muted(`${forms.length} forms`, cx)),
      )
      .child(div().h(1).w_full().bg(cx.theme().colors.border))
      .child(
        v_flex()
          .flex_1()
          .min_h_0()
          .overflow_y_scroll()
          .py(SPACE.sm)
          .children(forms.map((form) => this.formRow(form, cx))),
      );
  }

  /** @param {Context} cx */
  render(cx) {
    return v_flex()
      .size_full()
      .bg(cx.theme().colors.background)
      .child(
        new TitleBar().child(
          h_flex()
            .items_center()
            .gap(SPACE.md)
            .child(
              div()
                .text_size(13)
                .font_semibold()
                .text_color(cx.theme().colors.foreground)
                .child("buwiz BIR eFiling"),
            )
            .child(muted(`${this.info.runtime} · ${this.info.os}/${this.info.arch}`, cx)),
        ),
      )
      .child(h_flex().flex_1().min_h_0().child(this.sidebar(cx)).child(this.content(cx)));
  }
}
