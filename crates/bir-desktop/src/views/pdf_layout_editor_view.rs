use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::notification::Notification;
use gpui_component::*;

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};
use bir_print::formtype::{FormField, FormType};
use std::path::PathBuf;

pub struct PdfLayoutEditorView {
    pub form_type: Option<FormType>,
    pub file_path: Option<PathBuf>,
    pub selected_field_idx: Option<usize>,
    pub current_page: usize,
    pub form_select: Entity<ComboboxState>,
    pub scroll_handle: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl PdfLayoutEditorView {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let formtypes_dir = Self::get_formtypes_dir();

        // Scan formtypes/ directory for available forms
        let mut available_forms = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&formtypes_dir) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type()
                    && ft.is_dir()
                    && let Ok(name) = entry.file_name().into_string()
                {
                    let mut check = entry.path();
                    check.push("formtype.json");
                    if check.exists() {
                        available_forms.push(name);
                    }
                }
            }
        }
        available_forms.sort();

        // Create combobox with available form names
        let form_select = cx.new(|cx| ComboboxState::new(available_forms.clone(), window, cx));

        // Auto-load first available form
        let (auto_form, auto_path) = if let Some(first) = available_forms.first() {
            let mut p = formtypes_dir.clone();
            p.push(first);
            p.push("formtype.json");
            if let Ok(content) = std::fs::read_to_string(&p)
                && let Ok(ft) = serde_json::from_str::<FormType>(&content)
            {
                // Pre-select the form in the combobox
                form_select.update(cx, |select, cx| {
                    select.set_selected_value(first, window, cx);
                });
                (Some(ft), Some(p))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Subscribe to combobox selection — auto-load on select
        let _subscriptions = vec![cx.subscribe(
            &form_select,
            |this: &mut Self, _state, event: &ComboboxEvent, cx| {
                if let Some(selected) = &event.selected {
                    // Check if it looks like a path
                    let path = if selected.starts_with('/') || selected.starts_with("~/") {
                        PathBuf::from(Self::expand_tilde(selected))
                    } else {
                        let mut p = Self::get_formtypes_dir();
                        p.push(selected);
                        p.push("formtype.json");
                        p
                    };
                    this.load_file(path, cx);
                }
            },
        )];

        Self {
            form_type: auto_form,
            file_path: auto_path,
            selected_field_idx: None,
            current_page: 1,
            form_select,
            scroll_handle: ScrollHandle::new(),
            _subscriptions,
        }
    }

    fn get_formtypes_dir() -> PathBuf {
        let current_exe = std::env::current_exe().unwrap_or_default();
        if current_exe.to_string_lossy().contains("Contents/MacOS") {
            current_exe
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("Resources/formtypes")
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../formtypes")
        }
    }

    fn expand_tilde(path: &str) -> String {
        if let Some(rest) = path.strip_prefix("~/")
            && let Ok(home) = std::env::var("HOME")
        {
            return format!("{}/{}", home, rest);
        }
        path.to_string()
    }

    pub fn load_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(ft) = serde_json::from_str::<FormType>(&content)
        {
            self.form_type = Some(ft);
            self.file_path = Some(path);
            self.selected_field_idx = None;
            self.current_page = 1;
            cx.notify();
        }
    }

    pub fn save_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::WindowExt;
        if let (Some(ft), Some(path)) = (&self.form_type, &self.file_path) {
            if ft.save_to_file(path).is_ok() {
                window.push_notification(
                    Notification::success("Saved".to_string())
                        .message("Updated formtype.json".to_string()),
                    cx,
                );
            } else {
                window.push_notification(
                    Notification::error("Error".to_string()).message("Failed to save".to_string()),
                    cx,
                );
            }
        }
    }

    fn render_field_box(
        &self,
        idx: usize,
        field: &FormField,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self.selected_field_idx == Some(idx);
        let border_color = if is_selected {
            cx.theme().primary
        } else {
            cx.theme().danger
        };
        let bg_color = if is_selected {
            cx.theme().primary.opacity(0.3)
        } else {
            cx.theme().danger.opacity(0.1)
        };

        let width = field.cell_w.unwrap_or(20.0);
        let height = field.size.unwrap_or(10.0) + 4.0;

        div()
            .id(format!("field_box_{}", idx))
            .absolute()
            .top(px(field.y as f32))
            .left(px(field.x as f32))
            .w(px(width as f32))
            .h(px(height as f32))
            .bg(bg_color)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _ev, _window, cx| {
                this.selected_field_idx = Some(idx);
                cx.notify();
            }))
            .child(
                div()
                    .absolute()
                    .top(px(-12.0))
                    .text_xs()
                    .text_color(border_color)
                    .child(field.key.clone()),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_nudge_row(
        &self,
        label: &'static str,
        idx: usize,
        value: f64,
        dec_id: &str,
        inc_id: &str,
        mutator: fn(&mut FormField, f64),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div().flex().gap_2().items_center().child(label).child(
            div()
                .flex()
                .gap_1()
                .child(
                    Button::new(dec_id.to_string())
                        .label("-1")
                        .on_click(cx.listener(move |this, _ev, _window, cx| {
                            if let Some(ft) = &mut this.form_type
                                && let Some(f) = ft.fields.get_mut(idx)
                            {
                                mutator(f, -1.0);
                                cx.notify();
                            }
                        })),
                )
                .child(
                    div()
                        .w(px(40.0))
                        .flex()
                        .justify_center()
                        .child(format!("{:.1}", value)),
                )
                .child(
                    Button::new(inc_id.to_string())
                        .label("+1")
                        .on_click(cx.listener(move |this, _ev, _window, cx| {
                            if let Some(ft) = &mut this.form_type
                                && let Some(f) = ft.fields.get_mut(idx)
                            {
                                mutator(f, 1.0);
                                cx.notify();
                            }
                        })),
                ),
        )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(ft) = &self.form_type else {
            return div().into_any_element();
        };

        let fields_list = ft
            .fields
            .iter()
            .enumerate()
            .filter(|(_, f)| f.page == self.current_page)
            .map(|(idx, field)| {
                let is_selected = self.selected_field_idx == Some(idx);
                let bg = if is_selected {
                    cx.theme().muted
                } else {
                    cx.theme().background
                };
                div()
                    .id(format!("sidebar_field_{}", idx))
                    .p_2()
                    .bg(bg)
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().muted))
                    .on_click(cx.listener(move |this, _ev, _window, cx| {
                        this.selected_field_idx = Some(idx);
                        cx.notify();
                    }))
                    .child(field.key.clone())
            })
            .collect::<Vec<_>>();

        let selected_field_editor = if let Some(idx) = self.selected_field_idx {
            if let Some(field) = ft.fields.get(idx) {
                div()
                    .p_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .child(format!("Selected: {}", field.key)),
                    )
                    .child(self.render_nudge_row(
                        "X:",
                        idx,
                        field.x,
                        &format!("x_dec_{}", idx),
                        &format!("x_inc_{}", idx),
                        |f, d| f.x += d,
                        cx,
                    ))
                    .child(self.render_nudge_row(
                        "Y:",
                        idx,
                        field.y,
                        &format!("y_dec_{}", idx),
                        &format!("y_inc_{}", idx),
                        |f, d| f.y += d,
                        cx,
                    ))
                    .child(self.render_nudge_row(
                        "W:",
                        idx,
                        field.cell_w.unwrap_or(20.0),
                        &format!("w_dec_{}", idx),
                        &format!("w_inc_{}", idx),
                        |f, d| f.cell_w = Some(f.cell_w.unwrap_or(20.0) + d),
                        cx,
                    ))
                    .child(self.render_nudge_row(
                        "H:",
                        idx,
                        field.size.unwrap_or(10.0),
                        &format!("h_dec_{}", idx),
                        &format!("h_inc_{}", idx),
                        |f, d| f.size = Some(f.size.unwrap_or(10.0) + d),
                        cx,
                    ))
                    .into_any_element()
            } else {
                div().into_any_element()
            }
        } else {
            div().into_any_element()
        };

        div()
            .w(px(300.0))
            .h_full()
            .border_l_1()
            .border_color(cx.theme().border)
            .flex()
            .flex_col()
            .child(selected_field_editor)
            .child(
                div()
                    .id("fields_sidebar_list")
                    .flex_grow()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .children(fields_list),
            )
            .into_any_element()
    }
}

impl Render for PdfLayoutEditorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let max_page = self
            .form_type
            .as_ref()
            .map(|ft| ft.page_count())
            .unwrap_or(1);

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .p_4()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_center()
                    // Combobox — same component used for RDO field
                    .child(div().w(px(300.0)).child(Combobox::new(&self.form_select)))
                    // Page navigation (only show when a form is loaded)
                    .when(self.form_type.is_some(), |this| {
                        this.child(
                            div()
                                .flex()
                                .gap_2()
                                .items_center()
                                .ml_4()
                                .child("Page:")
                                .child(Button::new("prev_page").label("<").on_click(cx.listener(
                                    |this, _ev, _window, cx| {
                                        if this.current_page > 1 {
                                            this.current_page -= 1;
                                            cx.notify();
                                        }
                                    },
                                )))
                                .child(
                                    div()
                                        .w(px(40.0))
                                        .flex()
                                        .justify_center()
                                        .child(format!("{} / {}", self.current_page, max_page)),
                                )
                                .child(Button::new("next_page").label(">").on_click(cx.listener(
                                    move |this, _ev, _window, cx| {
                                        if this.current_page < max_page {
                                            this.current_page += 1;
                                            cx.notify();
                                        }
                                    },
                                ))),
                        )
                    }),
            )
            .child(
                Button::new("save")
                    .label("Save Layout")
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.save_file(window, cx);
                    })),
            );

        let body = if let Some(ft) = &self.form_type {
            let fields_canvas = ft
                .fields
                .iter()
                .enumerate()
                .filter(|(_, f)| f.page == self.current_page)
                .map(|(idx, field)| self.render_field_box(idx, field, cx).into_any_element())
                .collect::<Vec<_>>();

            let svg_path = if let Some(p) = &self.file_path {
                if let Some(parent) = p.parent() {
                    let path = parent.join(format!("pages/page{}.svg", self.current_page));
                    if path.exists() { Some(path) } else { None }
                } else {
                    None
                }
            } else {
                None
            };

            let bg_canvas = if let Some(svg) = svg_path {
                img(svg)
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            } else {
                div().bg(cx.theme().background).into_any_element()
            };

            div()
                .flex()
                .flex_row()
                .size_full()
                .child(
                    div()
                        .id("interactive_canvas_scroll")
                        .flex_grow()
                        .h_full()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll_handle)
                        .child(
                            div()
                                .relative()
                                .w(px(ft.page_width as f32))
                                .h(px(ft.page_height as f32))
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_sm()
                                .m_8()
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .size_full()
                                        .child(bg_canvas),
                                )
                                .children(fields_canvas),
                        ),
                )
                .child(self.render_sidebar(cx))
        } else {
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .size_full()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .text_color(cx.theme().muted_foreground)
                        .child("No layout forms found."),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Add a form layout directory under formtypes/<FormID>/ containing a formtype.json file."),
                )
        };

        div()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .flex()
            .flex_col()
            .child(header)
            .child(body)
    }
}
