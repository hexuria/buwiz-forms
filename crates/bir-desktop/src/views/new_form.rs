use gpui::*;
use gpui_component::*;
use bir_core::profile::TaxpayerProfile;

pub struct NewFormView {
    form_id: Option<String>,
    profile: Option<TaxpayerProfile>,
}

impl NewFormView {
    pub fn new(_window: &mut Window, _cx: &mut Context<'_, Self>) -> Self {
        Self {
            form_id: None,
            profile: None,
        }
    }

    pub fn set_form(&mut self, form_id: String, profile: TaxpayerProfile, cx: &mut Context<Self>) {
        self.form_id = Some(form_id);
        self.profile = Some(profile);
        cx.notify();
    }
}

impl Render for NewFormView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let Some(form_id) = &self.form_id else {
            return div().child("No form selected.").into_any_element();
        };

        let Some(profile) = &self.profile else {
            return div().child("No profile selected.").into_any_element();
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_12()
            .gap_8()
            .child(
                div().flex().flex_col().gap_2().child(
                    div().text_3xl().font_weight(FontWeight::BLACK).text_color(cx.theme().primary).child(format!("Drafting Form {}", form_id))
                ).child(
                    div().text_base().text_color(cx.theme().muted_foreground).child("Background information has been pre-filled from your profile vault.")
                )
            )
            .child(
                div()
                    .w_full()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .p_8()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div().text_xl().font_weight(FontWeight::BOLD).text_color(cx.theme().foreground).child("Part I - Background Information")
                    )
                    .child(
                        div().flex().gap_8().child(
                            div().flex_1().flex().flex_col().gap_1().child(div().text_xs().text_color(cx.theme().muted_foreground).child("1. TIN")).child(div().font_weight(FontWeight::BOLD).child(profile.tin.full()))
                        ).child(
                            div().flex_1().flex().flex_col().gap_1().child(div().text_xs().text_color(cx.theme().muted_foreground).child("2. RDO Code")).child(div().font_weight(FontWeight::BOLD).child(profile.rdo_code.clone()))
                        )
                    )
                    .child(
                        div().flex().flex_col().gap_1().child(div().text_xs().text_color(cx.theme().muted_foreground).child("3. Taxpayer Name")).child(div().font_weight(FontWeight::BOLD).child(profile.full_name.clone()))
                    )
                    .child(
                        div().flex().flex_col().gap_1().child(div().text_xs().text_color(cx.theme().muted_foreground).child("4. Registered Address")).child(div().font_weight(FontWeight::BOLD).child(profile.registered_address.clone()))
                    )
                    .child(
                        div().flex().gap_8().child(
                            div().flex_1().flex().flex_col().gap_1().child(div().text_xs().text_color(cx.theme().muted_foreground).child("5. Zip Code")).child(div().font_weight(FontWeight::BOLD).child(profile.zip_code.clone()))
                        ).child(
                            div().flex_1().flex().flex_col().gap_1().child(div().text_xs().text_color(cx.theme().muted_foreground).child("6. Telephone Number")).child(div().font_weight(FontWeight::BOLD).child(profile.phone.clone()))
                        )
                    )
                    .child(
                        div().flex().flex_col().gap_1().child(div().text_xs().text_color(cx.theme().muted_foreground).child("7. Email Address")).child(div().font_weight(FontWeight::BOLD).child(profile.email.clone()))
                    )
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_4()
                    .mt_4()
                    .child(
                        gpui_component::button::Button::new("cancel_form")
                            .label("Cancel")
                    )
                    .child(
                        gpui_component::button::Button::new("next_step")
                            .label("Continue to Part II")
                    )
            )
            .into_any_element()
    }
}
