use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::notification::Notification;
use gpui_component::*;

use bir_print::formtype::{FormField, FormType};
use std::path::PathBuf;

pub struct PdfLayoutEditorView {
    pub form_type: Option<FormType>,
    pub file_path: Option<PathBuf>,
    pub selected_field_idx: Option<usize>,
    pub current_page: usize,
    pub combo_input: Entity<InputState>,
    pub suggestions: Vec<String>,
    pub suggestion_idx: Option<usize>,
    pub show_suggestions: bool,
    pub available_forms: Vec<String>,
    pub scroll_handle: ScrollHandle,
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{}/{}", home, rest);
    }
    path.to_string()
}

fn compute_suggestions(query: &str, available_forms: &[String]) -> Vec<String> {
    if query.starts_with('/') || query.starts_with("~/") {
        // Path mode: list .json files in the directory
        let expanded = expand_tilde(query);
        let path = std::path::Path::new(&expanded);
        let (dir, prefix) = if path.is_dir() {
            (path.to_path_buf(), String::new())
        } else {
            let dir = path.parent().unwrap_or(std::path::Path::new("/"));
            let prefix = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            (dir.to_path_buf(), prefix)
        };

        let Ok(entries) = std::fs::read_dir(&dir) else {
            return vec![];
        };

        let mut results: Vec<String> = entries
            .flatten()
            .filter(|e| {
                let Ok(ft) = e.file_type() else {
                    return false;
                };
                if ft.is_dir() {
                    // Show directories so user can navigate deeper
                    let name = e.file_name().to_string_lossy().to_lowercase();
                    return prefix.is_empty() || name.starts_with(&prefix);
                }
                if !ft.is_file() {
                    return false;
                }
                let name = e.file_name().to_string_lossy().to_lowercase();
                let is_json = name.ends_with(".json");
                is_json && (prefix.is_empty() || name.starts_with(&prefix))
            })
            .map(|e| {
                let full = dir.join(e.file_name());
                let display = full.to_string_lossy().to_string();
                let Ok(ft) = e.file_type() else {
                    return display;
                };
                if ft.is_dir() {
                    format!("{}/", display)
                } else {
                    display
                }
            })
            .collect();
        results.sort();
        results.truncate(20);
        results
    } else {
        // Form mode: filter built-in forms
        let query_lower = query.to_lowercase();
        available_forms
            .iter()
            .filter(|name| {
                query_lower.is_empty() || name.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }
}

impl PdfLayoutEditorView {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        // Scan formtypes/ directory
        let mut available_forms = Vec::new();
        if let Ok(entries) = std::fs::read_dir("formtypes") {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type()
                    && ft.is_dir()
                    && let Ok(name) = entry.file_name().into_string()
                {
                    // Only include dirs that actually contain formtype.json
                    let mut check = entry.path();
                    check.push("formtype.json");
                    if check.exists() {
                        available_forms.push(name);
                    }
                }
            }
        }
        available_forms.sort();

        // Auto-load first available form
        let (auto_form, auto_path) = if let Some(first) = available_forms.first() {
            let mut p = std::env::current_dir().unwrap_or_default();
            p.push("formtypes");
            p.push(first);
            p.push("formtype.json");
            if let Ok(content) = std::fs::read_to_string(&p) {
                if let Ok(ft) = serde_json::from_str::<FormType>(&content) {
                    (Some(ft), Some(p))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        let default_text = available_forms.first().cloned().unwrap_or_default();
        let combo_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search forms or type path (/...)")
                .default_value(default_text)
        });

        // Subscribe to input events for filtering
        let forms_clone = available_forms.clone();
        cx.subscribe_in(
            &combo_input,
            window,
            move |this: &mut Self, _entity, event: &InputEvent, _window, cx| match event {
                InputEvent::Change => {
                    let query = this.combo_input.read(cx).value().to_string();
                    this.suggestions = compute_suggestions(&query, &forms_clone);
                    this.suggestion_idx = None;
                    this.show_suggestions = !this.suggestions.is_empty();
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    if let Some(idx) = this.suggestion_idx
                        && let Some(s) = this.suggestions.get(idx).cloned()
                    {
                        this.apply_suggestion(&s, _window, cx);
                        return;
                    }
                    // Try loading whatever is in the input
                    this.try_load(_window, cx);
                }
                InputEvent::Focus => {
                    let query = this.combo_input.read(cx).value().to_string();
                    this.suggestions = compute_suggestions(&query, &this.available_forms);
                    this.show_suggestions = !this.suggestions.is_empty();
                    cx.notify();
                }
                InputEvent::Blur => {
                    // Small delay to allow click on suggestion to fire first
                    // We'll hide via on_mouse_down_out instead
                }
            },
        )
        .detach();

        Self {
            form_type: auto_form,
            file_path: auto_path,
            selected_field_idx: None,
            current_page: 1,
            combo_input,
            suggestions: available_forms.clone(),
            suggestion_idx: None,
            show_suggestions: false,
            available_forms,
            scroll_handle: ScrollHandle::new(),
        }
    }

    fn apply_suggestion(&mut self, suggestion: &str, window: &mut Window, cx: &mut Context<Self>) {
        // If suggestion ends with /, it's a directory — just update the input text
        if suggestion.ends_with('/') {
            self.combo_input.update(cx, |input, cx| {
                input.set_value(suggestion, window, cx);
            });
            self.suggestions = compute_suggestions(suggestion, &self.available_forms);
            self.suggestion_idx = None;
            cx.notify();
            return;
        }

        self.combo_input.update(cx, |input, cx| {
            input.set_value(suggestion, window, cx);
        });
        self.show_suggestions = false;
        self.suggestion_idx = None;

        let path = if suggestion.starts_with('/') || suggestion.starts_with("~/") {
            PathBuf::from(expand_tilde(suggestion))
        } else {
            let mut p = std::env::current_dir().unwrap_or_default();
            p.push("formtypes");
            p.push(suggestion);
            p.push("formtype.json");
            p
        };
        self.load_file(path, cx);
    }

    fn try_load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.combo_input.read(cx).value().to_string();
        if raw.trim().is_empty() {
            return;
        }

        let path = if raw.starts_with('/') || raw.starts_with("~/") {
            PathBuf::from(expand_tilde(&raw))
        } else {
            let mut p = std::env::current_dir().unwrap_or_default();
            p.push("formtypes");
            p.push(&raw);
            p.push("formtype.json");
            p
        };

        if !path.exists() {
            use gpui_component::WindowExt;
            window.push_notification(
                Notification::error("Not Found".to_string())
                    .message(format!("File does not exist: {}", path.display())),
                cx,
            );
            return;
        }
        if path.is_dir() {
            use gpui_component::WindowExt;
            window.push_notification(
                Notification::warning("Invalid".to_string())
                    .message("Path is a directory, not a file".to_string()),
                cx,
            );
            return;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            use gpui_component::WindowExt;
            window.push_notification(
                Notification::warning("Wrong Type".to_string())
                    .message("Only .json layout files are supported".to_string()),
                cx,
            );
            return;
        }

        self.show_suggestions = false;
        self.load_file(path, cx);
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
            use gpui_component::WindowExt;
            if ft.save_to_file(path).is_ok() {
                window.push_notification(
                    Notification::success("Saved".to_string())
                        .message("Updated formtype.json".to_string()),
                    cx,
                );
            } else {
                window.push_notification(
                    Notification::error("Error".to_string())
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
        div()
            .flex()
            .gap_2()
            .items_center()
            .child(label)
            .child(
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

    fn render_combo_suggestions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.show_suggestions || self.suggestions.is_empty() {
            return div().into_any_element();
        }

        div()
            .absolute()
            .top(px(36.0))
            .left_0()
            .w(px(400.0))
            .max_h(px(200.0))
            .overflow_hidden()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .shadow_md()
            .children(
                self.suggestions
                    .iter()
                    .enumerate()
                    .map(|(i, suggestion)| {
                        let is_highlighted = self.suggestion_idx == Some(i);
                        let bg = if is_highlighted {
                            cx.theme().accent
                        } else {
                            cx.theme().background
                        };
                        let fg = if is_highlighted {
                            cx.theme().accent_foreground
                        } else {
                            cx.theme().foreground
                        };
                        let s = suggestion.clone();
                        div()
                            .id(format!("suggestion_{}", i))
                            .px_3()
                            .py_2()
                            .bg(bg)
                            .text_color(fg)
                            .text_sm()
                            .cursor_pointer()
                            .hover(|style| style.bg(cx.theme().muted))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _ev, window, cx| {
                                    this.apply_suggestion(&s, window, cx);
                                }),
                            )
                            .child(suggestion.clone())
                    })
                    .collect::<Vec<_>>(),
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
                    .child(
                        // ComboBox: single input + floating suggestions
                        div()
                            .id("combobox_container")
                            .relative()
                            .child(
                                Input::new(&self.combo_input)
                                    .w(px(400.0))
                                    .prefix(Icon::new(IconName::Search).small())
                                    .cleanable(true),
                            )
                            .child(self.render_combo_suggestions(cx))
                            .on_mouse_down_out(cx.listener(|this, _ev, _window, cx| {
                                this.show_suggestions = false;
                                cx.notify();
                            })),
                    )
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
                    if path.exists() {
                        Some(path)
                    } else {
                        None
                    }
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
                .items_center()
                .justify_center()
                .size_full()
                .child("No formtype layouts found. Add a formtypes/<FormID>/formtype.json directory.")
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
