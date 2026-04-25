use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::*;
use gpui_component::*;
use std::sync::{Arc, Mutex};

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};
use crate::components::date_input::{DateInput, DateInputEvent, DateInputState};

use crate::components::tin_input::TinInput;
use bir_core::db::Database;
use bir_core::naming::Tin;
use bir_core::profile::{TaxpayerProfile, TaxpayerType};
use bir_core::reference::get_all_rdos;
use bir_core::validation::{ValidationError, validate_profile};

pub enum ProfileEvent {
    Saved,
}

type SearchSelect = SelectState<SearchableVec<String>>;

pub struct ProfileManagerView {
    db: Arc<Mutex<Database>>,
    tin_input: Entity<TinInput>,
    rdo_select: Entity<ComboboxState>,
    type_select: Entity<ComboboxState>,
    line_of_business: Entity<InputState>,
    name_input: Entity<InputState>,
    address_input: Entity<InputState>,
    zip_input: Entity<InputState>,
    tel_input: Entity<InputState>,
    email_input: Entity<InputState>,
    business_start_input: Entity<DateInputState>,
    is_vat_registered: bool,
    editing_id: Option<i64>,
    errors: Vec<ValidationError>,
    save_message: Option<String>,
    rdo_options: Vec<String>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ProfileEvent> for ProfileManagerView {}

impl ProfileManagerView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let tin_input = cx.new(|cx| TinInput::new(window, cx));
        let rdo_options = get_all_rdos()
            .into_iter()
            .map(|r| format!("{} - {}", r.code, r.description))
            .collect::<Vec<_>>();

        let rdo_select = cx.new(|cx| ComboboxState::new(rdo_options.clone(), window, cx));
        let type_select = cx.new(|cx| {
            ComboboxState::new(
                vec![
                    "Individual".to_string(),
                    "Corporation".to_string(),
                    "Partnership".to_string(),
                ],
                window,
                cx,
            )
        });

        let line_of_business =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. SOFTWARE DEVELOPMENT"));
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Taxpayer's Name (Last Name, First Name, Middle Name)")
        });
        let address_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Registered Address"));
        let zip_input = cx.new(|cx| InputState::new(window, cx).placeholder("Zip Code"));
        let tel_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Mobile or Telephone No."));
        let email_input = cx.new(|cx| InputState::new(window, cx).placeholder("Email Address"));
        let business_start_input = cx.new(|cx| DateInputState::new(window, cx));

        let subscriptions = vec![
            cx.subscribe(&tin_input, Self::on_tin_event),
            cx.subscribe_in(&name_input, window, Self::on_input_event),
            cx.subscribe(&rdo_select, Self::on_combobox_event),
            cx.subscribe(&type_select, Self::on_combobox_event),
            cx.subscribe(&business_start_input, Self::on_date_event),
        ];

        Self {
            db,
            tin_input,
            rdo_select,
            type_select,
            line_of_business,
            name_input,
            address_input,
            zip_input,
            tel_input,
            email_input,
            business_start_input,
            is_vat_registered: false,
            editing_id: None,
            errors: Vec::new(),
            save_message: None,
            rdo_options,
            _subscriptions: subscriptions,
        }
    }

    pub fn reset_for_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_id = None;
        self.is_vat_registered = false;
        self.errors.clear();
        self.save_message = None;
        self.tin_input.update(cx, |tin, cx| tin.clear(window, cx));
        for input in [
            &self.line_of_business,
            &self.name_input,
            &self.address_input,
            &self.zip_input,
            &self.tel_input,
            &self.email_input,
        ] {
            input.update(cx, |input, cx| input.set_value(String::new(), window, cx));
        }
        self.business_start_input
            .update(cx, |input, cx| input.set_date(None, window, cx));
        self.rdo_select.update(cx, |select, cx| {
            select.set_selected_value("", window, cx);
        });
        self.type_select.update(cx, |select, cx| {
            select.set_selected_value("Individual", window, cx);
        });
        cx.notify();
    }

    pub fn edit_profile(
        &mut self,
        profile: TaxpayerProfile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editing_id = profile.id;
        self.is_vat_registered = profile.is_vat_registered;
        self.errors.clear();
        self.save_message = None;
        self.tin_input
            .update(cx, |tin, cx| tin.set_from_tin(&profile.tin, window, cx));

        self.name_input.update(cx, |input, cx| {
            input.set_value(profile.full_name.clone(), window, cx)
        });
        self.address_input.update(cx, |input, cx| {
            input.set_value(profile.registered_address.clone(), window, cx)
        });
        self.zip_input.update(cx, |input, cx| {
            input.set_value(profile.zip_code.clone(), window, cx)
        });
        self.tel_input.update(cx, |input, cx| {
            input.set_value(profile.phone.clone(), window, cx)
        });
        self.email_input.update(cx, |input, cx| {
            input.set_value(profile.email.clone(), window, cx)
        });
        self.line_of_business.update(cx, |input, cx| {
            input.set_value(profile.line_of_business.clone(), window, cx)
        });
        self.business_start_input.update(cx, |input, cx| {
            input.set_date(profile.business_start_date, window, cx)
        });

        let rdo_value = self
            .rdo_options
            .iter()
            .find(|option| option.starts_with(&profile.rdo_code))
            .cloned()
            .unwrap_or(profile.rdo_code.clone());
        self.rdo_select.update(cx, |select, cx| {
            select.set_selected_value(&rdo_value, window, cx);
        });
        let type_value = taxpayer_type_label(&profile.taxpayer_type).to_string();
        self.type_select.update(cx, |select, cx| {
            select.set_selected_value(&type_value, window, cx);
        });

        cx.notify();
    }

    fn on_tin_event(
        &mut self,
        _state: Entity<TinInput>,
        _event: &gpui_component::input::InputEvent,
        _cx: &mut Context<Self>,
    ) {
        // No-op since sync_card is removed
    }

    fn on_input_event(
        &mut self,
        _state: &Entity<InputState>,
        _event: &InputEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // No-op since sync_card is removed
    }

    fn on_combobox_event(
        &mut self,
        _state: Entity<ComboboxState>,
        _event: &ComboboxEvent,
        _cx: &mut Context<Self>,
    ) {
        // No-op since sync_card is removed
    }

    fn on_date_event(
        &mut self,
        _state: Entity<DateInputState>,
        _event: &DateInputEvent,
        _cx: &mut Context<Self>,
    ) {
        // Handle date event if needed
    }

    fn current_profile(&self, cx: &mut Context<Self>) -> TaxpayerProfile {
        let tin_val = self.tin_input.read(cx).formatted_value(cx);
        let tin_clean = tin_val.replace("-", "");
        let tin = Tin {
            segment1: tin_clean.get(0..3).unwrap_or("").to_string(),
            segment2: tin_clean.get(3..6).unwrap_or("").to_string(),
            segment3: tin_clean.get(6..9).unwrap_or("").to_string(),
            branch: tin_clean.get(9..).unwrap_or("").to_string(),
        };

        let type_val = self.type_select.read(cx).selected_value(cx);
        let taxpayer_type = match type_val.as_str() {
            "Corporation" => TaxpayerType::Corporation,
            "Partnership" => TaxpayerType::Partnership,
            _ => TaxpayerType::Individual,
        };

        let rdo_code = self
            .rdo_select
            .read(cx)
            .selected_value(cx)
            .split(" - ")
            .next()
            .unwrap_or("")
            .to_string();

        let business_start_date = self.business_start_input.read(cx).date;

        TaxpayerProfile {
            id: self.editing_id,
            full_name: self.name_input.read(cx).value().trim().to_string(),
            tin,
            rdo_code,
            line_of_business: self.line_of_business.read(cx).value().trim().to_string(),
            registered_address: self.address_input.read(cx).value().trim().to_string(),
            zip_code: self.zip_input.read(cx).value().trim().to_string(),
            phone: self.tel_input.read(cx).value().trim().to_string(),
            email: self.email_input.read(cx).value().trim().to_string(),
            default_form_type: "2551Qv2018".into(),
            taxpayer_type,
            is_vat_registered: self.is_vat_registered,
            business_start_date,
        }
    }

    fn save_profile(&mut self, cx: &mut Context<Self>) {
        let profile = self.current_profile(cx);
        self.errors = validate_profile(&profile);
        if !self
            .business_start_input
            .read(cx)
            .value(cx)
            .trim()
            .is_empty()
            && profile.business_start_date.is_none()
        {
            self.errors.push(ValidationError::new(
                "business_start_date",
                "Date must use YYYY-MM-DD",
            ));
        }

        if !self.errors.is_empty() {
            self.save_message = None;
            cx.notify();
            return;
        }

        if let Ok(db) = self.db.lock() {
            match db.save_profile(profile) {
                Ok(saved) => {
                    self.editing_id = saved.id;
                    self.save_message = Some("Profile saved".to_string());
                    cx.emit(ProfileEvent::Saved);
                }
                Err(err) => {
                    self.save_message = Some(format!("Save failed: {err}"));
                }
            }
        }
        cx.notify();
    }

    fn field_label(label: &str, cx: &Context<Self>) -> gpui::Div {
        div()
            .text_sm()
            .font_weight(FontWeight::BOLD)
            .text_color(cx.theme().foreground)
            .mb_2()
            .child(label.to_string())
    }

    fn field_error(&self, field: &'static str, _cx: &Context<Self>) -> gpui::Div {
        let text = self
            .errors
            .iter()
            .find(|err| err.field == field)
            .map(|err| err.message.clone())
            .unwrap_or_default();
        div()
            .h_5()
            .text_xs()
            .text_color(gpui::rgba(0xff6b6bff))
            .child(text)
    }
}

impl Render for ProfileManagerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let title = if self.editing_id.is_some() {
            "Edit Taxpayer Profile"
        } else {
            "Taxpayer Profile Setup"
        };

        let type_val = self.type_select.read(cx).selected_value(cx);
        let is_individual = type_val == "Individual";
        let date_label = if is_individual {
            "Birth Date"
        } else {
            "Business Start Date"
        };

        div()
            .id("profile-scroll")
            .size_full()
            .overflow_y_scroll()
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
                    let is_enter = event.keystroke.key == "enter";
                    let is_modifier =
                        event.keystroke.modifiers.platform || event.keystroke.modifiers.control;
                    if is_enter && is_modifier {
                        this.save_profile(cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_center()
                    .w_full()
                    .p_12()
                    .child(
                    div()
                        .flex()
                        .flex_row()
                        .w_full()
                        .max_w(px(1100.))
                        .gap_16()
                        .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .w(px(550.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_3xl()
                                            .font_weight(FontWeight::BLACK)
                                            .text_color(cx.theme().foreground)
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .text_base()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(
                                                "Required information is used to pre-fill 2551Q.",
                                            ),
                                    ),
                            )
                            .child(self.tin_input.clone().into_any_element())
                            .child(
                                div()
                                    .flex()
                                    .gap_4()
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Self::field_label(
                                                "Revenue District Office (RDO)",
                                                cx,
                                            ))
                                            .child(Combobox::new(&self.rdo_select))
                                            .child(self.field_error("rdo_code", cx)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Self::field_label("Taxpayer Type", cx))
                                            .child(Combobox::new(&self.type_select)),
                                    ),
                            )
                            .child(
                                div()
                                    .child(Self::field_label("Line of Business", cx))
                                    .child(Input::new(&self.line_of_business))
                                    .child(self.field_error("line_of_business", cx)),
                            )
                            .child(
                                div()
                                    .child(Self::field_label("Taxpayer's Name", cx))
                                    .child(Input::new(&self.name_input))
                                    .child(self.field_error("full_name", cx)),
                            )
                            .child(
                                div()
                                    .child(Self::field_label("Registered Address", cx))
                                    .child(Input::new(&self.address_input))
                                    .child(self.field_error("registered_address", cx)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_4()
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Self::field_label("Zip Code", cx))
                                            .child(Input::new(&self.zip_input))
                                            .child(self.field_error("zip_code", cx)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Self::field_label("Phone / Telephone No.", cx))
                                            .child(Input::new(&self.tel_input))
                                            .child(self.field_error("phone", cx)),
                                    ),
                            )
                            .child(
                                div()
                                    .child(Self::field_label("Email Address", cx))
                                    .child(Input::new(&self.email_input))
                                    .child(self.field_error("email", cx)),
                            )
                            .child(
                                div()
                                    .child(Self::field_label(date_label, cx))
                                    .child(DateInput::new(&self.business_start_input))
                                    .child(self.field_error("business_start_date", cx)),
                            )
                            .child(
                                div()
                                    .id("vat_toggle")
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.is_vat_registered = !this.is_vat_registered;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .w_4()
                                            .h_4()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .bg(if self.is_vat_registered {
                                                cx.theme().primary
                                            } else {
                                                cx.theme().background
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(if self.is_vat_registered {
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().primary_foreground)
                                                    .child("✓")
                                            } else {
                                                div()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().foreground)
                                            .child("VAT registered taxpayer"),
                                    ),
                            )
                            .child(
                                div()
                                    .mt_4()
                                    .flex()
                                    .items_center()
                                    .gap_4()
                                    .child(
                                        gpui_component::button::Button::new("save_profile")
                                            .label("Save Profile")
                                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                                this.save_profile(cx);
                                            })),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(self.save_message.clone().unwrap_or_default()),
                                    ),
                            ),
                    ),
                ),
            )
    }
}

fn taxpayer_type_label(taxpayer_type: &TaxpayerType) -> &'static str {
    match taxpayer_type {
        TaxpayerType::Individual => "Individual",
        TaxpayerType::Corporation => "Corporation",
        TaxpayerType::Partnership => "Partnership",
    }
}
