use gpui::prelude::*;
use gpui::*;
use gpui_component::*;

/// A reusable field label for BIR forms.
pub fn field_label<V: 'static>(label: &str, cx: &Context<V>) -> gpui::Div {
    div()
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(cx.theme().muted_foreground)
        .mb_1()
        .child(label.to_string())
}

/// A standard readonly field display for profile-prefilled data.
pub fn readonly_field<V: 'static>(
    label: &str,
    value: &str,
    error: Option<&String>,
    cx: &Context<V>,
) -> gpui::Div {
    let text_col = if error.is_some() {
        cx.theme().danger
    } else {
        cx.theme().foreground
    };

    let mut d = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(field_label(label, cx))
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_color(text_col)
                .text_sm()
                .child(if value.is_empty() {
                    "—".to_string()
                } else {
                    value.to_string()
                }),
        );

    if let Some(err_msg) = error {
        d = d.child(
            div()
                .text_xs()
                .text_color(cx.theme().danger)
                .child(err_msg.clone()),
        );
    }

    d
}

/// Standardized currency display (₱)
pub fn currency_display<V: 'static>(amount: f64, cx: &Context<V>) -> gpui::Div {
    div()
        .font_weight(FontWeight::BOLD)
        .text_color(cx.theme().primary)
        .text_sm()
        .child(format!("\u{20b1} {:.2}", amount))
}

pub struct TaxpayerInfoProps<'a> {
    pub tin: &'a str,
    pub tin_err: Option<&'a String>,
    pub rdo: &'a str,
    pub rdo_err: Option<&'a String>,
    pub name: &'a str,
    pub name_err: Option<&'a String>,
    pub address: &'a str,
    pub address_err: Option<&'a String>,
    pub zip: &'a str,
    pub zip_err: Option<&'a String>,
    pub contact: &'a str,
    pub contact_err: Option<&'a String>,
    pub email: &'a str,
    pub email_err: Option<&'a String>,
}

/// A standard Taxpayer Information section layout
pub fn taxpayer_info_section<V: 'static>(props: TaxpayerInfoProps<'_>, cx: &Context<V>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .gap_8()
                .child(readonly_field("1. TIN", props.tin, props.tin_err, cx).flex_1())
                .child(readonly_field("2. RDO Code", props.rdo, props.rdo_err, cx).flex_1()),
        )
        .child(readonly_field(
            "3. Taxpayer's Name",
            props.name,
            props.name_err,
            cx,
        ))
        .child(readonly_field(
            "4. Registered Address",
            props.address,
            props.address_err,
            cx,
        ))
        .child(
            div()
                .flex()
                .gap_8()
                .child(readonly_field("5. Zip Code", props.zip, props.zip_err, cx).flex_1())
                .child(readonly_field("6. Contact Number", props.contact, props.contact_err, cx).flex_1()),
        )
        .child(readonly_field(
            "7. Email Address",
            props.email,
            props.email_err,
            cx,
        ))
}

/// Standardized Penalty Summary section (Surcharge, Interest, Compromise, Total)
pub fn penalty_summary_section<V: 'static>(
    surcharge: f64,
    interest: f64,
    compromise: f64,
    total_penalties: f64,
    total_amount_payable: f64,
    cx: &Context<V>,
) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child("Add: Penalties"),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_x_4()
                        .gap_y_2()
                        .pl_6()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Surcharge"),
                        )
                        .child(currency_display(surcharge, cx)),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_x_4()
                        .gap_y_2()
                        .pl_6()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Interest"),
                        )
                        .child(currency_display(interest, cx)),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_x_4()
                        .gap_y_2()
                        .pl_6()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Compromise"),
                        )
                        .child(currency_display(compromise, cx)),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_x_4()
                        .gap_y_2()
                        .pl_6()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().foreground)
                                .child("Total Penalties"),
                        )
                        .child(currency_display(total_penalties, cx)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_x_4()
                .gap_y_2()
                .justify_between()
                .items_center()
                .pt_4()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BLACK)
                        .text_color(cx.theme().foreground)
                        .child("Total Amount Payable / (Overpayment)"),
                )
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::BLACK)
                        .text_color(if total_amount_payable > 0.0 {
                            cx.theme().primary
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(format!("\u{20b1} {:.2}", total_amount_payable)),
                ),
        )
}

#[allow(clippy::too_many_arguments)]
pub fn form_accordion<V: 'static, F>(
    id: &str,
    label: &str,
    is_expanded: bool,
    is_valid: bool,
    has_error: bool,
    on_click: F,
    content: AnyElement,
    cx: &Context<V>,
) -> gpui::Div
where
    F: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
{
    let mut card = div()
        .bg(cx.theme().secondary)
        .rounded_xl()
        .border_1()
        .border_color(if has_error {
            cx.theme().danger
        } else {
            cx.theme().border
        })
        .p_6()
        .flex()
        .flex_col()
        .child(
            div()
                .id(id.to_string())
                .flex()
                .justify_between()
                .items_center()
                .cursor_pointer()
                .w_full()
                .p_2()
                .rounded_md()
                .hover(|style| style.bg(cx.theme().muted.opacity(0.5)))
                .on_click(on_click)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .w_5()
                                .h_5()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(if is_valid {
                                    div()
                                        .text_color(gpui::rgba(0x22c55eff))
                                        .font_weight(FontWeight::BLACK)
                                        .text_lg()
                                        .child("✓")
                                        .into_any_element()
                                } else {
                                    div().into_any_element()
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .whitespace_normal()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().foreground)
                                .child(label.to_string()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w_6()
                        .h_6()
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(cx.theme().muted)
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(if is_expanded { "▲" } else { "▼" }),
                        ),
                ),
        );

    if is_expanded {
        card = card.child(
            div()
                .mt_4()
                .pt_4()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(content),
        );
    }
    
    card
}

pub fn computation_row_readonly<V: 'static>(label: &str, amount: f64, is_total: bool, cx: &Context<V>) -> gpui::Div {
    let mut amount_div = div()
        .font_weight(FontWeight::BOLD)
        .text_sm()
        .child(format!("\u{20b1} {:.2}", amount));
        
    amount_div = if is_total {
        amount_div.text_color(if amount > 0.0 { cx.theme().primary } else { cx.theme().muted_foreground })
    } else {
        amount_div.text_color(cx.theme().primary)
    };

    div()
        .flex()
        .flex_wrap()
        .gap_x_4()
        .gap_y_2()
        .justify_between()
        .items_center()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(label.to_string()),
        )
        .child(amount_div)
}

pub struct ComputationRowInputProps<'a> {
    pub label: &'a str,
    pub input_component: AnyElement,
    pub error_message: Option<&'a String>,
    pub locked_message: Option<&'a str>,
    pub is_mobile: bool,
}

pub fn computation_row_input<V: 'static>(props: ComputationRowInputProps<'_>, cx: &Context<V>) -> gpui::Div {
    let is_locked = props.locked_message.is_some();
    let text_color = if is_locked { cx.theme().muted_foreground } else { cx.theme().foreground };
    let opacity_val = if is_locked { 0.4 } else { 1.0 };

    let row = if props.is_mobile {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .text_color(text_color)
                            .child(props.label.to_string()),
                    )
                    .children(props.locked_message.map(|m| {
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(m.to_string())
                    })),
            )
            .child(
                div()
                    .w_full()
                    .opacity(opacity_val)
                    .child(props.input_component),
            )
    } else {
        div()
            .flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(text_color)
                            .child(props.label.to_string()),
                    )
                    .children(props.locked_message.map(|m| {
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(m.to_string())
                    })),
            )
            .child(
                div()
                    .w(px(180.))
                    .opacity(opacity_val)
                    .child(props.input_component),
            )
    };

    let mut container = div().flex().flex_col().gap_1().child(row);

    if let Some(err_msg) = props.error_message {
        container = container.child(
            div()
                .text_xs()
                .text_color(cx.theme().danger)
                .child(err_msg.to_string()),
        );
    }

    container
}

pub struct ScheduleRowProps<'a> {
    pub atc: String,
    pub description: String,
    pub amount_label: String,
    pub rate: String,
    pub tax_due: f64,
    pub error_message: Option<&'a String>,
    pub input_component: AnyElement,
}

pub struct AtcScheduleTableProps<'a> {
    pub title: &'a str,
    pub amount_col_label: &'a str,
    pub is_mobile: bool,
    pub rows: Vec<ScheduleRowProps<'a>>,
}

/// A generic table for ATC schedules (e.g. Schedule 1 of 2551Q, 2550M, etc.)
pub fn atc_schedule_table<V: 'static>(props: AtcScheduleTableProps<'_>, cx: &Context<V>) -> gpui::Div {
    let mut container = div()
        .flex()
        .flex_col()
        .gap_4();

    if !props.title.is_empty() {
        container = container.child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().primary)
                .child(props.title.to_string()),
        );
    }

    if !props.is_mobile && !props.rows.is_empty() {
        container = container.child(
            div()
                .flex()
                .gap_2()
                .pb_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .w(px(80.))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child("ATC"),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child("DESCRIPTION"),
                )
                .child(
                    div()
                        .w(px(140.))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child(props.amount_col_label.to_string()),
                )
                .child(
                    div()
                        .w(px(50.))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child("RATE"),
                )
                .child(
                    div()
                        .w(px(120.))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child("TAX DUE (₱)"),
                ),
        );
    }

    for row in props.rows {
        let row_content = if props.is_mobile {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .py_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().primary)
                                .child(row.atc.to_string()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().foreground)
                                .child(row.description.to_string()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child(row.amount_label.to_string()),
                        )
                        .child(div().w_full().child(row.input_component)),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child("RATE"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(row.rate.to_string()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().muted_foreground)
                                .child("TAX DUE (₱)"),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().primary)
                                .text_sm()
                                .child(format!("{:.2}", row.tax_due)),
                        ),
                )
        } else {
            div()
                .flex()
                .gap_2()
                .items_center()
                .py_2()
                .child(
                    div()
                        .w(px(80.))
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().primary)
                        .child(row.atc.to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(cx.theme().foreground)
                        .child(row.description.to_string()),
                )
                .child(div().w(px(140.)).child(row.input_component))
                .child(
                    div()
                        .w(px(50.))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(row.rate.to_string()),
                )
                .child(
                    div()
                        .w(px(120.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().primary)
                        .text_sm()
                        .child(format!("{:.2}", row.tax_due)),
                )
        };

        let mut item = div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(row_content);

        if let Some(err_msg) = row.error_message {
            item = item.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .pb_2()
                    .child(err_msg.to_string()),
            );
        }

        container = container.child(item);
    }

    container
}
