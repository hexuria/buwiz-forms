use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::*;

use bir_print::formtype::{FormField, FormType};
use std::path::PathBuf;

pub struct PdfLayoutEditorView {
    pub form_type: Option<FormType>,
    pub file_path: Option<PathBuf>,
    pub selected_field_idx: Option<usize>,
    pub current_page: usize,
    pub input_path: Entity<InputState>,
    pub scroll_handle: ScrollHandle,
}

impl PdfLayoutEditorView {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let input_path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Path to formtype.json (e.g. formtypes/2551Qv2018/formtype.json)")
        });
        Self {
            form_type: None,
            file_path: None,
            selected_field_idx: None,
            current_page: 1,
            input_path,
            scroll_handle: ScrollHandle::new(),
        }
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
        if let (Some(ft), Some(path)) = (&self.form_type, &self.file_path) {
            if ft.save_to_file(path).is_ok() {
                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::success("Saved".to_string())
                        .message("Updated formtype.json".to_string()),
                    cx,
                );
            } else {
                use gpui_component::WindowExt;
                window.push_notification(
                    gpui_component::notification::Notification::error("Error".to_string())
                        .message("Failed to save".to_string()),
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
        let height = field.size.unwrap_or(10.0) + 4.0; // Approximation

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
                    .child(
                        div().flex().gap_2().items_center().child("X:").child(
                            div()
                                .flex()
                                .gap_1()
                                .child(Button::new(format!("x_dec_{}", idx)).label("-1").on_click(
                                    cx.listener(move |this, _ev, _window, cx| {
                                        if let Some(ft) = &mut this.form_type
                                            && let Some(f) = ft.fields.get_mut(idx)
                                        {
                                            f.x -= 1.0;
                                            cx.notify();
                                        }
                                    }),
                                ))
                                .child(
                                    div()
                                        .w(px(40.0))
                                        .flex()
                                        .justify_center()
                                        .child(format!("{:.1}", field.x)),
                                )
                                .child(Button::new(format!("x_inc_{}", idx)).label("+1").on_click(
                                    cx.listener(move |this, _ev, _window, cx| {
                                        if let Some(ft) = &mut this.form_type
                                            && let Some(f) = ft.fields.get_mut(idx)
                                        {
                                            f.x += 1.0;
                                            cx.notify();
                                        }
                                    }),
                                )),
                        ),
                    )
                    .child(
                        div().flex().gap_2().items_center().child("Y:").child(
                            div()
                                .flex()
                                .gap_1()
                                .child(Button::new(format!("y_dec_{}", idx)).label("-1").on_click(
                                    cx.listener(move |this, _ev, _window, cx| {
                                        if let Some(ft) = &mut this.form_type
                                            && let Some(f) = ft.fields.get_mut(idx)
                                        {
                                            f.y -= 1.0;
                                            cx.notify();
                                        }
                                    }),
                                ))
                                .child(
                                    div()
                                        .w(px(40.0))
                                        .flex()
                                        .justify_center()
                                        .child(format!("{:.1}", field.y)),
                                )
                                .child(Button::new(format!("y_inc_{}", idx)).label("+1").on_click(
                                    cx.listener(move |this, _ev, _window, cx| {
                                        if let Some(ft) = &mut this.form_type
                                            && let Some(f) = ft.fields.get_mut(idx)
                                        {
                                            f.y += 1.0;
                                            cx.notify();
                                        }
                                    }),
                                )),
                        ),
                    )
                    .child(
                        div().flex().gap_2().items_center().child("W:").child(
                            div()
                                .flex()
                                .gap_1()
                                .child(Button::new(format!("w_dec_{}", idx)).label("-1").on_click(
                                    cx.listener(move |this, _ev, _window, cx| {
                                        if let Some(ft) = &mut this.form_type
                                            && let Some(f) = ft.fields.get_mut(idx)
                                        {
                                            f.cell_w = Some(f.cell_w.unwrap_or(20.0) - 1.0);
                                            cx.notify();
                                        }
                                    }),
                                ))
                                .child(
                                    div()
                                        .w(px(40.0))
                                        .flex()
                                        .justify_center()
                                        .child(format!("{:.1}", field.cell_w.unwrap_or(20.0))),
                                )
                                .child(Button::new(format!("w_inc_{}", idx)).label("+1").on_click(
                                    cx.listener(move |this, _ev, _window, cx| {
                                        if let Some(ft) = &mut this.form_type
                                            && let Some(f) = ft.fields.get_mut(idx)
                                        {
                                            f.cell_w = Some(f.cell_w.unwrap_or(20.0) + 1.0);
                                            cx.notify();
                                        }
                                    }),
                                )),
                        ),
                    )
                    .child(
                        div().flex().gap_2().items_center().child("H:").child(
                            div()
                                .flex()
                                .gap_1()
                                .child(Button::new(format!("h_dec_{}", idx)).label("-1").on_click(
                                    cx.listener(move |this, _ev, _window, cx| {
                                        if let Some(ft) = &mut this.form_type
                                            && let Some(f) = ft.fields.get_mut(idx)
                                        {
                                            f.size = Some(f.size.unwrap_or(10.0) - 1.0);
                                            cx.notify();
                                        }
                                    }),
                                ))
                                .child(
                                    div()
                                        .w(px(40.0))
                                        .flex()
                                        .justify_center()
                                        .child(format!("{:.1}", field.size.unwrap_or(10.0))),
                                )
                                .child(Button::new(format!("h_inc_{}", idx)).label("+1").on_click(
                                    cx.listener(move |this, _ev, _window, cx| {
                                        if let Some(ft) = &mut this.form_type
                                            && let Some(f) = ft.fields.get_mut(idx)
                                        {
                                            f.size = Some(f.size.unwrap_or(10.0) + 1.0);
                                            cx.notify();
                                        }
                                    }),
                                )),
                        ),
                    )
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
                    .child(Input::new(&self.input_path).w(px(400.0)))
                    .child(Button::new("load").label("Load").on_click(cx.listener(
                        |this, _ev, _window, cx| {
                            let path_str = this.input_path.read(cx).value();
                            let mut p = std::env::current_dir().unwrap_or_default();
                            p.push(path_str.to_string());
                            this.load_file(p, cx);
                        },
                    )))
                    .child(
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
                                    .w(px(20.0))
                                    .flex()
                                    .justify_center()
                                    .child(format!("{}", self.current_page)),
                            )
                            .child(Button::new("next_page").label(">").on_click(cx.listener(
                                |this, _ev, _window, cx| {
                                    this.current_page += 1;
                                    cx.notify();
                                },
                            ))),
                    ),
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
                    // Interactive Canvas
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
                .items_center()
                .justify_center()
                .size_full()
                .child("No form loaded. Enter path and click Load.")
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
