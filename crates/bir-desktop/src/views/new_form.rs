use gpui::*;
use gpui_component::*;
use gpui_rsx::rsx;
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

        let muted = cx.theme().muted_foreground;

        // Bare flags (`flex_col`, `p_12`) expand to the identically-named GPUI
        // builder methods, so this is a 1:1 rewrite of the previous chain.
        // Deliberately *not* using rsx's Tailwind `class=`: there `p-12` means
        // `p(px(12.))`, whereas GPUI's `.p_12()` is 48px.
        let root = rsx! {
            <div flex flex_col size_full p_12 gap_8>
                <div flex flex_col gap_2>
                    <div text_3xl font_weight={FontWeight::BLACK} text_color={cx.theme().primary}>
                        {format!("Drafting Form {}", form_id)}
                    </div>
                    <div text_base text_color={muted}>
                        {"Background information has been pre-filled from your profile vault."}
                    </div>
                </div>

                <div
                    w_full
                    bg={cx.theme().background}
                    border_1
                    border_color={cx.theme().border}
                    rounded_xl
                    p_8
                    flex
                    flex_col
                    gap_6
                >
                    <div text_xl font_weight={FontWeight::BOLD} text_color={cx.theme().foreground}>
                        {"Part I - Background Information"}
                    </div>

                    <div flex gap_8>
                        <div flex_1 flex flex_col gap_1>
                            <div text_xs text_color={muted}>{"1. TIN"}</div>
                            <div font_weight={FontWeight::BOLD}>{profile.tin.full()}</div>
                        </div>
                        <div flex_1 flex flex_col gap_1>
                            <div text_xs text_color={muted}>{"2. RDO Code"}</div>
                            <div font_weight={FontWeight::BOLD}>{profile.rdo_code.clone()}</div>
                        </div>
                    </div>

                    <div flex flex_col gap_1>
                        <div text_xs text_color={muted}>{"3. Taxpayer Name"}</div>
                        <div font_weight={FontWeight::BOLD}>{profile.full_name.clone()}</div>
                    </div>

                    <div flex flex_col gap_1>
                        <div text_xs text_color={muted}>{"4. Registered Address"}</div>
                        <div font_weight={FontWeight::BOLD}>{profile.registered_address.clone()}</div>
                    </div>

                    <div flex gap_8>
                        <div flex_1 flex flex_col gap_1>
                            <div text_xs text_color={muted}>{"5. Zip Code"}</div>
                            <div font_weight={FontWeight::BOLD}>{profile.zip_code.clone()}</div>
                        </div>
                        <div flex_1 flex flex_col gap_1>
                            <div text_xs text_color={muted}>{"6. Telephone Number"}</div>
                            <div font_weight={FontWeight::BOLD}>{profile.phone.clone()}</div>
                        </div>
                    </div>

                    <div flex flex_col gap_1>
                        <div text_xs text_color={muted}>{"7. Email Address"}</div>
                        <div font_weight={FontWeight::BOLD}>{profile.email.clone()}</div>
                    </div>
                </div>

                // gpui-component widgets compose as ordinary `{expr}` children:
                // anything implementing `IntoElement` becomes a `.child(..)` call.
                <div flex justify_end gap_4 mt_4>
                    {gpui_component::button::Button::new("cancel_form").label("Cancel")}
                    {gpui_component::button::Button::new("next_step").label("Continue to Part II")}
                </div>
            </div>
        };

        root.into_any_element()
    }
}
