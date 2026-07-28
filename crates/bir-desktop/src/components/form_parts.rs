use gpui::prelude::*;
use gpui::*;
use gpui_component::*;
use gpui_rsx::rsx;

/// A reusable field label for BIR forms.
pub fn field_label<V: 'static>(label: &str, cx: &Context<V>) -> gpui::Div {
    rsx! {
        <div
            text_xs
            font_weight={FontWeight::BOLD}
            text_color={cx.theme().muted_foreground}
            mb_1
        >
            {label.to_string()}
        </div>
    }
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

    let mut d = rsx! {
        <div flex flex_col gap_1>
            {field_label(label, cx)}
            {div()
                .font_weight(FontWeight::BOLD)
                .text_color(text_col)
                .text_sm()
                .child(if value.is_empty() {
                    "—".to_string()
                } else {
                    value.to_string()
                })}
        </div>
    };

    if let Some(err_msg) = error {
        d = d.child(rsx! {
            <div text_xs text_color={cx.theme().danger}>
                {err_msg.clone()}
            </div>
        });
    }

    d
}

/// Standardized currency display (₱)
pub fn currency_display<V: 'static>(amount: f64, cx: &Context<V>) -> gpui::Div {
    rsx! {
        <div font_weight={FontWeight::BOLD} text_color={cx.theme().primary} text_sm>
            {format!("\u{20b1} {:.2}", amount)}
        </div>
    }
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
pub fn taxpayer_info_section<V: 'static>(
    props: TaxpayerInfoProps<'_>,
    cx: &Context<V>,
) -> gpui::Div {
    rsx! {
        <div flex flex_col gap_4>
            <div flex gap_8>
                {readonly_field("1. TIN", props.tin, props.tin_err, cx).flex_1()}
                {readonly_field("2. RDO Code", props.rdo, props.rdo_err, cx).flex_1()}
            </div>
            {readonly_field(
                "3. Taxpayer's Name",
                props.name,
                props.name_err,
                cx,
            )}
            {readonly_field(
                "4. Registered Address",
                props.address,
                props.address_err,
                cx,
            )}
            <div flex gap_8>
                {readonly_field("5. Zip Code", props.zip, props.zip_err, cx).flex_1()}
                {readonly_field("6. Contact Number", props.contact, props.contact_err, cx)
                    .flex_1()}
            </div>
            {readonly_field(
                "7. Email Address",
                props.email,
                props.email_err,
                cx,
            )}
        </div>
    }
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
    rsx! {
        <div flex flex_col gap_4>
            <div flex flex_col gap_2>
                <div text_sm text_color={cx.theme().foreground}>
                    {"Add: Penalties"}
                </div>
                <div flex flex_wrap gap_x_4 gap_y_2 pl_6 justify_between items_center>
                    <div text_sm text_color={cx.theme().muted_foreground}>
                        {"Surcharge"}
                    </div>
                    {currency_display(surcharge, cx)}
                </div>
                <div flex flex_wrap gap_x_4 gap_y_2 pl_6 justify_between items_center>
                    <div text_sm text_color={cx.theme().muted_foreground}>
                        {"Interest"}
                    </div>
                    {currency_display(interest, cx)}
                </div>
                <div flex flex_wrap gap_x_4 gap_y_2 pl_6 justify_between items_center>
                    <div text_sm text_color={cx.theme().muted_foreground}>
                        {"Compromise"}
                    </div>
                    {currency_display(compromise, cx)}
                </div>
                <div flex flex_wrap gap_x_4 gap_y_2 pl_6 justify_between items_center>
                    <div
                        text_sm
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {"Total Penalties"}
                    </div>
                    {currency_display(total_penalties, cx)}
                </div>
            </div>
            <div
                flex
                flex_wrap
                gap_x_4
                gap_y_2
                justify_between
                items_center
                pt_4
                border_t_1
                border_color={cx.theme().border}
            >
                <div
                    text_sm
                    font_weight={FontWeight::BLACK}
                    text_color={cx.theme().foreground}
                >
                    {"Total Amount Payable / (Overpayment)"}
                </div>
                <div
                    text_2xl
                    font_weight={FontWeight::BLACK}
                    text_color={if total_amount_payable > 0.0 {
                        cx.theme().primary
                    } else {
                        cx.theme().muted_foreground
                    }}
                >
                    {format!("\u{20b1} {:.2}", total_amount_payable)}
                </div>
            </div>
        </div>
    }
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
    let mut card = rsx! {
        <div
            bg={cx.theme().secondary}
            rounded_xl
            border_1
            border_color={if has_error {
                cx.theme().danger
            } else {
                cx.theme().border
            }}
            p_6
            flex
            flex_col
        >
            <div
                id={id.to_string()}
                flex
                justify_between
                items_center
                cursor_pointer
                w_full
                p_2
                rounded_md
                hover={|style| style.bg(cx.theme().muted.opacity(0.5))}
                on_click={on_click}
            >
                <div flex items_center gap_3 flex_1 min_w_0>
                    {div()
                        .w_5()
                        .h_5()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(if is_valid {
                            div()
                                .text_color(crate::theme::success_on_tint(cx.theme()))
                                .font_weight(FontWeight::BLACK)
                                .text_lg()
                                .child("✓")
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        })}
                    <div
                        flex_1
                        min_w_0
                        text_sm
                        whitespace_normal
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().foreground}
                    >
                        {label.to_string()}
                    </div>
                </div>
                <div
                    flex
                    items_center
                    justify_center
                    w_6
                    h_6
                    flex_shrink_0
                    rounded_full
                    bg={cx.theme().muted}
                >
                    {div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(if is_expanded { "▲" } else { "▼" })}
                </div>
            </div>
        </div>
    };

    if is_expanded {
        card = card.child(rsx! {
            <div mt_4 pt_4 border_t_1 border_color={cx.theme().border}>
                {content}
            </div>
        });
    }

    card
}

pub fn computation_row_readonly<V: 'static>(
    label: &str,
    amount: f64,
    is_total: bool,
    cx: &Context<V>,
) -> gpui::Div {
    let mut amount_div = div()
        .font_weight(FontWeight::BOLD)
        .text_sm()
        .child(format!("\u{20b1} {:.2}", amount));

    amount_div = if is_total {
        amount_div.text_color(if amount > 0.0 {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground
        })
    } else {
        amount_div.text_color(cx.theme().primary)
    };

    rsx! {
        <div flex flex_wrap gap_x_4 gap_y_2 justify_between items_center>
            <div text_sm text_color={cx.theme().foreground}>
                {label.to_string()}
            </div>
            {amount_div}
        </div>
    }
}

pub struct ComputationRowInputProps<'a> {
    pub label: &'a str,
    pub input_component: AnyElement,
    pub error_message: Option<&'a String>,
    pub locked_message: Option<&'a str>,
    pub is_mobile: bool,
}

pub fn computation_row_input<V: 'static>(
    props: ComputationRowInputProps<'_>,
    cx: &Context<V>,
) -> gpui::Div {
    let is_locked = props.locked_message.is_some();
    let text_color = if is_locked {
        cx.theme().muted_foreground
    } else {
        cx.theme().foreground
    };
    let opacity_val = if is_locked { 0.4 } else { 1.0 };

    let row = if props.is_mobile {
        rsx! {
            <div flex flex_col gap_2>
                <div flex flex_col gap_1>
                    <div text_sm text_color={text_color}>
                        {props.label.to_string()}
                    </div>
                    {...props.locked_message.map(|m| {
                        rsx! {
                            <div text_xs text_color={cx.theme().muted_foreground}>
                                {m.to_string()}
                            </div>
                        }
                    })}
                </div>
                <div w_full opacity={opacity_val}>
                    {props.input_component}
                </div>
            </div>
        }
    } else {
        rsx! {
            <div flex justify_between items_center>
                <div flex items_center gap_2>
                    <div text_sm text_color={text_color}>
                        {props.label.to_string()}
                    </div>
                    {...props.locked_message.map(|m| {
                        rsx! {
                            <div text_xs text_color={cx.theme().muted_foreground}>
                                {m.to_string()}
                            </div>
                        }
                    })}
                </div>
                <div w={px(180.)} opacity={opacity_val}>
                    {props.input_component}
                </div>
            </div>
        }
    };

    let mut container = rsx! {
        <div flex flex_col gap_1>
            {row}
        </div>
    };

    if let Some(err_msg) = props.error_message {
        container = container.child(rsx! {
            <div text_xs text_color={cx.theme().danger}>
                {err_msg.to_string()}
            </div>
        });
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
    pub action_component: Option<AnyElement>,
}

pub struct AtcScheduleTableProps<'a> {
    pub title: &'a str,
    pub amount_col_label: &'a str,
    pub is_mobile: bool,
    pub rows: Vec<ScheduleRowProps<'a>>,
}

/// A generic table for ATC schedules (e.g. Schedule 1 of 2551Q, 2550M, etc.)
pub fn atc_schedule_table<V: 'static>(
    props: AtcScheduleTableProps<'_>,
    cx: &Context<V>,
) -> gpui::Div {
    let mut container = div().flex().flex_col().gap_4();
    let show_actions = props.rows.iter().any(|row| row.action_component.is_some());

    if !props.title.is_empty() {
        container = container.child(rsx! {
            <div
                text_sm
                font_weight={FontWeight::BOLD}
                text_color={cx.theme().primary}
            >
                {props.title.to_string()}
            </div>
        });
    }

    if !props.is_mobile && !props.rows.is_empty() {
        container = container.child(
            div()
                .flex()
                .gap_2()
                .pb_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(rsx! {
                    <div
                        w={px(80.)}
                        text_xs
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().muted_foreground}
                    >
                        {"ATC"}
                    </div>
                })
                .child(rsx! {
                    <div
                        flex_1
                        text_xs
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().muted_foreground}
                    >
                        {"DESCRIPTION"}
                    </div>
                })
                .child(rsx! {
                    <div
                        w={px(140.)}
                        text_xs
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().muted_foreground}
                    >
                        {props.amount_col_label.to_string()}
                    </div>
                })
                .child(rsx! {
                    <div
                        w={px(50.)}
                        text_xs
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().muted_foreground}
                    >
                        {"RATE"}
                    </div>
                })
                .child(rsx! {
                    <div
                        w={px(120.)}
                        text_xs
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().muted_foreground}
                    >
                        {"TAX DUE (₱)"}
                    </div>
                })
                .when(show_actions, |header| {
                    header.child(rsx! {
                        <div
                            w={px(72.)}
                            text_xs
                            font_weight={FontWeight::BOLD}
                            text_color={cx.theme().muted_foreground}
                        >
                            {"ACTION"}
                        </div>
                    })
                }),
        );
    }

    for row in props.rows {
        let ScheduleRowProps {
            atc,
            description,
            amount_label,
            rate,
            tax_due,
            error_message,
            input_component,
            action_component,
        } = row;
        let row_content = if props.is_mobile {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .py_4()
                .child(rsx! {
                    <div flex flex_col gap_1>
                        <div
                            text_sm
                            font_weight={FontWeight::BOLD}
                            text_color={cx.theme().primary}
                        >
                            {atc}
                        </div>
                        <div text_xs text_color={cx.theme().foreground}>
                            {description}
                        </div>
                    </div>
                })
                .child(rsx! {
                    <div flex flex_col gap_1>
                        <div
                            text_xs
                            font_weight={FontWeight::BOLD}
                            text_color={cx.theme().muted_foreground}
                        >
                            {amount_label}
                        </div>
                        <div w_full>
                            {input_component}
                        </div>
                    </div>
                })
                .child(rsx! {
                    <div flex justify_between items_center>
                        <div
                            text_xs
                            font_weight={FontWeight::BOLD}
                            text_color={cx.theme().muted_foreground}
                        >
                            {"RATE"}
                        </div>
                        <div text_sm text_color={cx.theme().muted_foreground}>
                            {rate}
                        </div>
                    </div>
                })
                .child(rsx! {
                    <div flex justify_between items_center>
                        <div
                            text_xs
                            font_weight={FontWeight::BOLD}
                            text_color={cx.theme().muted_foreground}
                        >
                            {"TAX DUE (₱)"}
                        </div>
                        <div
                            font_weight={FontWeight::BOLD}
                            text_color={cx.theme().primary}
                            text_sm
                        >
                            {format!("{tax_due:.2}")}
                        </div>
                    </div>
                })
                .when_some(action_component, |row, action| {
                    row.child(rsx! {
                        <div flex justify_end>
                            {action}
                        </div>
                    })
                })
        } else {
            div()
                .flex()
                .gap_2()
                .items_center()
                .py_2()
                .child(rsx! {
                    <div
                        w={px(80.)}
                        text_sm
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().primary}
                    >
                        {atc}
                    </div>
                })
                .child(rsx! {
                    <div flex_1 text_xs text_color={cx.theme().foreground}>
                        {description}
                    </div>
                })
                .child(rsx! {
                    <div w={px(140.)}>
                        {input_component}
                    </div>
                })
                .child(rsx! {
                    <div w={px(50.)} text_sm text_color={cx.theme().muted_foreground}>
                        {rate}
                    </div>
                })
                .child(rsx! {
                    <div
                        w={px(120.)}
                        font_weight={FontWeight::BOLD}
                        text_color={cx.theme().primary}
                        text_sm
                    >
                        {format!("{tax_due:.2}")}
                    </div>
                })
                .when_some(action_component, |row, action| {
                    row.child(rsx! {
                        <div w={px(72.)}>
                            {action}
                        </div>
                    })
                })
        };

        let mut item = rsx! {
            <div flex flex_col border_b_1 border_color={cx.theme().border}>
                {row_content}
            </div>
        };

        if let Some(err_msg) = error_message {
            item = item.child(rsx! {
                <div text_xs text_color={cx.theme().danger} pb_2>
                    {err_msg.to_string()}
                </div>
            });
        }

        container = container.child(item);
    }

    container
}
