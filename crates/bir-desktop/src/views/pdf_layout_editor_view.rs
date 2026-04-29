use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::notification::Notification;
use gpui_component::*;

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};
use bir_print::formtype::{FormField, FormType};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InteractionMode {
    None,
    Panning,
    DrawingNew,
    Moving,
    Resizing,
}

pub struct PdfLayoutEditorView {
    pub form_type: Option<FormType>,
    pub file_path: Option<PathBuf>,
    pub selected_field_idx: Option<usize>,
    pub current_page: usize,
    pub form_select: Entity<ComboboxState>,
    pub search_filter: Entity<InputState>,
    pub scroll_handle: ScrollHandle,
    _subscriptions: Vec<Subscription>,
    // Interactive Canvas State
    pub scale: f32,
    pub pan_offset: gpui::Point<f32>,
    pub drawing_field_idx: Option<usize>,
    pub drawing_start: Option<gpui::Point<f32>>,
    pub drawing_current: Option<gpui::Point<f32>>,
    pub interaction_mode: InteractionMode,
    pub interaction_start_pos: Option<gpui::Point<f32>>,
    pub interaction_start_rect: Option<gpui::Bounds<f32>>,
    pub pan_start_offset: Option<gpui::Point<f32>>,
}

impl PdfLayoutEditorView {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let formtypes_dir = Self::get_formtypes_dir();

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

        let form_select = cx.new(|cx| ComboboxState::new(available_forms.clone(), window, cx));
        let search_filter =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search fields..."));

        let (auto_form, auto_path) = if let Some(first) = available_forms.first() {
            let mut p = formtypes_dir.clone();
            p.push(first);
            p.push("formtype.json");
            if let Ok(content) = std::fs::read_to_string(&p)
                && let Ok(ft) = serde_json::from_str::<FormType>(&content)
            {
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

        let _subscriptions = vec![cx.subscribe(
            &form_select,
            |this: &mut Self, _state, event: &ComboboxEvent, cx| {
                if let Some(selected) = &event.selected {
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
            search_filter,
            scroll_handle: ScrollHandle::new(),
            _subscriptions,
            scale: 1.0,
            pan_offset: gpui::Point { x: 0.0, y: 0.0 },
            drawing_field_idx: None,
            drawing_start: None,
            drawing_current: None,
            interaction_mode: InteractionMode::None,
            interaction_start_pos: None,
            interaction_start_rect: None,
            pan_start_offset: None,
        }
    }

    fn get_formtypes_dir() -> PathBuf {
        crate::platform::find_resource_dir("formtypes")
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
            self.scale = 1.0;
            self.pan_offset = gpui::Point { x: 0.0, y: 0.0 };
            self.drawing_field_idx = None;
            self.interaction_mode = InteractionMode::None;
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

    fn check_collision(&self, idx: usize, field: &FormField, ft: &FormType) -> bool {
        let fx = field.x;
        let fy = field.y;
        let fw = field.cell_w.unwrap_or(20.0);
        let fh = field.size.unwrap_or(10.0) + 4.0;

        for (i, other) in ft.fields.iter().enumerate() {
            if i == idx || other.page != self.current_page {
                continue;
            }
            let ox = other.x;
            let oy = other.y;
            let ow = other.cell_w.unwrap_or(20.0);
            let oh = other.size.unwrap_or(10.0) + 4.0;

            if fx < ox + ow && fx + fw > ox && fy < oy + oh && fy + fh > oy {
                return true;
            }
        }
        false
    }

    fn render_field_box(
        &self,
        idx: usize,
        field: &FormField,
        has_collision: bool,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self.selected_field_idx == Some(idx);
        let is_drawing = self.drawing_field_idx == Some(idx);

        let (border_color, bg_color) = if is_drawing {
            (cx.theme().info, cx.theme().info.opacity(0.4))
        } else if has_collision {
            (cx.theme().danger, cx.theme().danger.opacity(0.4))
        } else if is_selected {
            (cx.theme().warning, cx.theme().warning.opacity(0.4))
        } else {
            (cx.theme().border, cx.theme().secondary.opacity(0.2))
        };

        let is_drawing_new = is_drawing && self.interaction_mode == InteractionMode::DrawingNew;

        let (x, y, w, h) = if is_drawing_new
            && let Some(start) = self.drawing_start
            && let Some(current) = self.drawing_current
        {
            let min_x = start.x.min(current.x);
            let min_y = start.y.min(current.y);
            let max_x = start.x.max(current.x);
            let max_y = start.y.max(current.y);
            (min_x, min_y, max_x - min_x, max_y - min_y)
        } else {
            let width = field.cell_w.unwrap_or(20.0);
            let height = field.size.unwrap_or(10.0) + 4.0;
            (field.x as f32, field.y as f32, width as f32, height as f32)
        };

        div()
            .id(format!("field_box_{}", idx))
            .absolute()
            .top(px(y * scale))
            .left(px(x * scale))
            .w(px(w * scale))
            .h(px(h * scale))
            .bg(bg_color)
            .border_1()
            .border_color(border_color)
            .when(is_drawing, |this| {
                // Add resize handle on bottom right if in drawing/editing mode
                this.child(
                    div()
                        .absolute()
                        .bottom(px(-4.0))
                        .right(px(-4.0))
                        .size(px(8.0))
                        .bg(cx.theme().info)
                        .border_1()
                        .border_color(cx.theme().background)
                        .cursor_pointer() // Resize cursor isn't strictly available in standard gpui CursorStyle enum so we use pointer
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                                this.interaction_mode = InteractionMode::Resizing;
                                let scaled_x =
                                    (f32::from(ev.position.x) - this.pan_offset.x) / this.scale;
                                let scaled_y =
                                    (f32::from(ev.position.y) - this.pan_offset.y) / this.scale;
                                this.interaction_start_pos = Some(gpui::Point {
                                    x: scaled_x,
                                    y: scaled_y,
                                });
                                if let Some(ft) = &this.form_type {
                                    if let Some(f) = ft.fields.get(idx) {
                                        this.interaction_start_rect = Some(gpui::Bounds {
                                            origin: gpui::Point {
                                                x: f.x as f32,
                                                y: f.y as f32,
                                            },
                                            size: gpui::Size {
                                                width: f.cell_w.unwrap_or(20.0) as f32,
                                                height: (f.size.unwrap_or(10.0) + 4.0) as f32,
                                            },
                                        });
                                    }
                                }
                            }),
                        ),
                )
            })
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                    cx.stop_propagation();
                    if this.drawing_field_idx == Some(idx) {
                        // Start Moving
                        this.interaction_mode = InteractionMode::Moving;
                        let scaled_x = (f32::from(ev.position.x) - this.pan_offset.x) / this.scale;
                        let scaled_y = (f32::from(ev.position.y) - this.pan_offset.y) / this.scale;
                        this.interaction_start_pos = Some(gpui::Point {
                            x: scaled_x,
                            y: scaled_y,
                        });
                        if let Some(ft) = &this.form_type {
                            if let Some(f) = ft.fields.get(idx) {
                                this.interaction_start_rect = Some(gpui::Bounds {
                                    origin: gpui::Point {
                                        x: f.x as f32,
                                        y: f.y as f32,
                                    },
                                    size: gpui::Size {
                                        width: f.cell_w.unwrap_or(20.0) as f32,
                                        height: (f.size.unwrap_or(10.0) + 4.0) as f32,
                                    },
                                });
                            }
                        }
                    } else {
                        this.selected_field_idx = Some(idx);
                        cx.notify();
                    }
                }),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(ft) = &self.form_type else {
            return div().into_any_element();
        };

        let filter = self.search_filter.read(cx).value().to_lowercase();

        let fields_list = ft
            .fields
            .iter()
            .enumerate()
            .filter(|(_, f)| f.page == self.current_page)
            .filter(|(_, f)| filter.is_empty() || f.key.to_lowercase().contains(&filter))
            .map(|(idx, field)| {
                let is_selected = self.selected_field_idx == Some(idx);
                let is_drawing = self.drawing_field_idx == Some(idx);
                let has_collision = self.check_collision(idx, field, ft);

                let bg = if is_selected {
                    cx.theme().muted
                } else if has_collision {
                    cx.theme().danger.opacity(0.1)
                } else {
                    cx.theme().background
                };

                let label_color = if has_collision {
                    cx.theme().danger
                } else {
                    cx.theme().foreground
                };

                div()
                    .id(format!("sidebar_field_{}", idx))
                    .p_2()
                    .bg(bg)
                    .flex()
                    .justify_between()
                    .items_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().muted))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                            cx.stop_propagation();
                            this.selected_field_idx = Some(idx);

                            // If we are currently drawing, keep drawing mode but switch the field
                            if this.drawing_field_idx.is_some() {
                                this.drawing_field_idx = Some(idx);
                            }

                            if let Some(ft) = &this.form_type {
                                if let Some(f) = ft.fields.get(idx) {
                                    this.scale = 2.0;
                                    let fx = f.x as f32;
                                    let fy = f.y as f32;
                                    this.pan_offset = gpui::Point {
                                        x: -fx * 2.0 + 300.0,
                                        y: -fy * 2.0 + 300.0,
                                    };
                                }
                            }
                            cx.notify();
                        }),
                    )
                    .child(div().text_color(label_color).child(field.key.clone()))
                    .child(
                        div()
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                                    cx.stop_propagation();
                                    if this.drawing_field_idx == Some(idx) {
                                        // Toggle off if already drawing this field
                                        this.drawing_field_idx = None;
                                        this.interaction_mode = InteractionMode::None;
                                    } else {
                                        this.selected_field_idx = Some(idx);
                                        this.drawing_field_idx = Some(idx);
                                        this.interaction_mode = InteractionMode::None;
                                        this.drawing_start = None;
                                        this.drawing_current = None;

                                        // Auto-zoom to it if it has valid bounds
                                        if let Some(ft) = &this.form_type {
                                            if let Some(f) = ft.fields.get(idx) {
                                                if f.x > 0.0 && f.y > 0.0 {
                                                    this.scale = 2.0;
                                                    let fx = f.x as f32;
                                                    let fy = f.y as f32;
                                                    this.pan_offset = gpui::Point {
                                                        x: -fx * 2.0 + 300.0,
                                                        y: -fy * 2.0 + 300.0,
                                                    };
                                                }
                                            }
                                        }
                                    }
                                    cx.notify();
                                }),
                            )
                            .child(
                                svg()
                                    .path(if is_drawing {
                                        "svg/check.svg"
                                    } else {
                                        "svg/target.svg"
                                    })
                                    .size(px(16.))
                                    .text_color(if is_drawing {
                                        cx.theme().success
                                    } else {
                                        cx.theme().muted_foreground
                                    }),
                            ),
                    )
            })
            .collect::<Vec<_>>();

        div()
            .w(px(300.0))
            .h_full()
            .border_l_1()
            .border_color(cx.theme().border)
            .flex()
            .flex_col()
            .child(
                div()
                    .p_4()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Input::new(&self.search_filter))
            )
            .when(self.drawing_field_idx.is_some(), |this| {
                this.child(
                    div()
                        .p_4()
                        .bg(cx.theme().info.opacity(0.1))
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().info)
                                .child("Edit Mode Active. You can move, resize, or redraw the selected field box. Click the checkmark to finish.")
                        )
                )
            })
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child(div().w(px(300.0)).child(Combobox::new(&self.form_select)))
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
                                )))
                                .child(Button::new("reset_view").label("Reset View").on_click(
                                    cx.listener(|this, _ev, _window, cx| {
                                        this.scale = 1.0;
                                        this.pan_offset = gpui::Point { x: 0.0, y: 0.0 };
                                        cx.notify();
                                    }),
                                )),
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
            let scale = self.scale;
            let pan = self.pan_offset;
            let is_editing = self.drawing_field_idx.is_some();

            let fields_canvas = ft
                .fields
                .iter()
                .enumerate()
                .filter(|(_, f)| f.page == self.current_page)
                .filter(|(idx, _)| {
                    // Hide other boxes if we are actively editing a specific one
                    !is_editing || self.drawing_field_idx == Some(*idx)
                })
                .map(|(idx, field)| {
                    let has_collision = self.check_collision(idx, field, ft);
                    self.render_field_box(idx, field, has_collision, scale, cx)
                        .into_any_element()
                })
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
                        .id("interactive_canvas_viewport")
                        .flex_grow()
                        .h_full()
                        .overflow_hidden()
                        .bg(cx.theme().muted)
                        .when(is_editing, |this| this.cursor(CursorStyle::Crosshair))
                        .on_scroll_wheel(cx.listener(
                            move |this, ev: &ScrollWheelEvent, _window, cx| {
                                // Only allow zooming/panning if not dragging
                                if this.interaction_mode != InteractionMode::None {
                                    return;
                                }

                                if ev.modifiers.shift || ev.modifiers.control || ev.modifiers.alt {
                                    let delta =
                                        f32::from(ev.delta.pixel_delta(Pixels::from(100.0)).y);
                                    let zoom_factor = 1.0 - (delta * 0.005);
                                    this.scale = (this.scale * zoom_factor).clamp(0.1, 10.0);
                                    cx.notify();
                                } else {
                                    let dx = f32::from(ev.delta.pixel_delta(Pixels::from(100.0)).x);
                                    let dy = f32::from(ev.delta.pixel_delta(Pixels::from(100.0)).y);
                                    this.pan_offset.x += dx;
                                    this.pan_offset.y += dy;
                                    cx.notify();
                                }
                            },
                        ))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                                if let Some(_) = this.drawing_field_idx {
                                    this.interaction_mode = InteractionMode::DrawingNew;
                                    let scaled_x =
                                        (f32::from(ev.position.x) - this.pan_offset.x) / this.scale;
                                    let scaled_y =
                                        (f32::from(ev.position.y) - this.pan_offset.y) / this.scale;
                                    let pt = gpui::Point {
                                        x: scaled_x,
                                        y: scaled_y,
                                    };
                                    this.drawing_start = Some(pt);
                                    this.drawing_current = Some(pt);
                                    cx.notify();
                                } else {
                                    this.selected_field_idx = None;
                                    cx.notify();
                                }
                            }),
                        )
                        .on_mouse_down(
                            MouseButton::Middle,
                            cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                                this.interaction_mode = InteractionMode::Panning;
                                this.pan_start_offset = Some(gpui::Point {
                                    x: f32::from(ev.position.x) - this.pan_offset.x,
                                    y: f32::from(ev.position.y) - this.pan_offset.y,
                                });
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Middle,
                            cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                                this.interaction_mode = InteractionMode::None;
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                                if this.interaction_mode == InteractionMode::DrawingNew {
                                    if let Some(idx) = this.drawing_field_idx {
                                        if let Some(start) = this.drawing_start
                                            && let Some(current) = this.drawing_current
                                        {
                                            let min_x = start.x.min(current.x);
                                            let min_y = start.y.min(current.y);
                                            let max_x = start.x.max(current.x);
                                            let max_y = start.y.max(current.y);
                                            let w = max_x - min_x;
                                            let h = max_y - min_y;

                                            // Ignore tiny accidental clicks
                                            if w > 5.0 && h > 5.0 {
                                                if let Some(ft) = &mut this.form_type {
                                                    if let Some(f) = ft.fields.get_mut(idx) {
                                                        f.x = min_x as f64;
                                                        f.y = min_y as f64;
                                                        f.cell_w = Some(w as f64);
                                                        f.size = Some((h as f64 - 4.0).max(1.0));
                                                    }
                                                }
                                            }
                                        }
                                        this.drawing_start = None;
                                        this.drawing_current = None;
                                    }
                                }

                                if this.interaction_mode != InteractionMode::Panning {
                                    this.interaction_mode = InteractionMode::None;
                                }
                                cx.notify();
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                            if this.interaction_mode == InteractionMode::Panning {
                                if let Some(start) = this.pan_start_offset {
                                    this.pan_offset.x = f32::from(ev.position.x) - start.x;
                                    this.pan_offset.y = f32::from(ev.position.y) - start.y;
                                    cx.notify();
                                }
                            } else if this.interaction_mode == InteractionMode::DrawingNew {
                                let scaled_x =
                                    (f32::from(ev.position.x) - this.pan_offset.x) / this.scale;
                                let scaled_y =
                                    (f32::from(ev.position.y) - this.pan_offset.y) / this.scale;
                                this.drawing_current = Some(gpui::Point {
                                    x: scaled_x,
                                    y: scaled_y,
                                });
                                cx.notify();
                            } else if this.interaction_mode == InteractionMode::Moving
                                || this.interaction_mode == InteractionMode::Resizing
                            {
                                if let Some(start_pos) = this.interaction_start_pos {
                                    if let Some(start_rect) = this.interaction_start_rect {
                                        let scaled_x = (f32::from(ev.position.x)
                                            - this.pan_offset.x)
                                            / this.scale;
                                        let scaled_y = (f32::from(ev.position.y)
                                            - this.pan_offset.y)
                                            / this.scale;
                                        let dx = scaled_x - start_pos.x;
                                        let dy = scaled_y - start_pos.y;

                                        if let Some(ft) = &mut this.form_type {
                                            if let Some(idx) = this.drawing_field_idx {
                                                if let Some(f) = ft.fields.get_mut(idx) {
                                                    if this.interaction_mode
                                                        == InteractionMode::Moving
                                                    {
                                                        f.x = (start_rect.origin.x + dx) as f64;
                                                        f.y = (start_rect.origin.y + dy) as f64;
                                                    } else if this.interaction_mode
                                                        == InteractionMode::Resizing
                                                    {
                                                        let new_w =
                                                            (start_rect.size.width + dx).max(5.0);
                                                        let new_h =
                                                            (start_rect.size.height + dy).max(5.0);
                                                        f.cell_w = Some(new_w as f64);
                                                        f.size = Some((new_h as f64) - 4.0);
                                                    }
                                                    cx.notify();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }))
                        .child(
                            div()
                                .absolute()
                                .top(px(pan.y))
                                .left(px(pan.x))
                                .w(px(ft.page_width as f32 * scale))
                                .h(px(ft.page_height as f32 * scale))
                                .border_1()
                                .border_color(cx.theme().border)
                                .shadow_sm()
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
