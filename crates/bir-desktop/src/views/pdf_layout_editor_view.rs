use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::input::{Input, InputState};
use gpui_component::notification::Notification;
use gpui_component::*;

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};
use bir_print::formtype::{Direction, FieldKind, FormField, FormType, WidgetSpec, WidgetType};
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
    pub new_field_input: Entity<InputState>,
    pub new_field_modal_open: bool,
    pub char_count_input: Entity<InputState>,
    pub char_count_modal_open: bool,
    pub scroll_handle: ScrollHandle,
    pub focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
    // Interactive Canvas State
    pub scale: f32,
    pub pan_offset: gpui::Point<f32>,
    pub is_edit_mode: bool,
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
        let new_field_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Enter field name"));
        let char_count_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Enter char count (0 to clear)"));

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

        let _subscriptions = vec![
            cx.subscribe(
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
            ),
            cx.subscribe(
                &new_field_input,
                |this: &mut Self, _input, event: &gpui_component::input::InputEvent, cx| {
                    if let gpui_component::input::InputEvent::PressEnter { .. } = event {
                        let field_name = _input.read(cx).value().to_string();
                        if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                            if let Some(ft) = &mut this.form_type {
                                if let Some(f) = ft.fields.get_mut(idx) {
                                    f.key = field_name.clone();
                                }
                            }
                        }
                        this.new_field_modal_open = false;
                        cx.notify();
                    }
                },
            ),
            cx.subscribe(
                &char_count_input,
                |this: &mut Self, _input, event: &gpui_component::input::InputEvent, cx| {
                    if let gpui_component::input::InputEvent::PressEnter { .. } = event {
                        let val = _input.read(cx).value().to_string();
                        if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                            if let Some(ft) = &mut this.form_type {
                                if let Some(f) = ft.fields.get_mut(idx) {
                                    if let Ok(count) = val.parse::<usize>() {
                                        if count == 0 {
                                            f.char_count = None;
                                        } else {
                                            f.char_count = Some(count);
                                        }
                                    } else if val.trim().is_empty() {
                                        f.char_count = None;
                                    }
                                }
                            }
                        }
                        this.char_count_modal_open = false;
                        cx.notify();
                    }
                },
            ),
        ];

        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);

        Self {
            form_type: auto_form,
            file_path: auto_path,
            selected_field_idx: None,
            current_page: 1,
            form_select,
            search_filter,
            new_field_input,
            new_field_modal_open: false,
            char_count_input,
            char_count_modal_open: false,
            scroll_handle: ScrollHandle::new(),
            focus_handle,
            _subscriptions,
            scale: 1.0,
            pan_offset: gpui::Point { x: 0.0, y: 0.0 },
            is_edit_mode: false,
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
            self.is_edit_mode = false;
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

    fn focus_box(&mut self, local_box_idx: usize, cx: &mut Context<Self>) {
        if let Some(ft) = &self.form_type {
            if let Some(idx) = self.selected_field_idx.or(self.drawing_field_idx) {
                let current_key = &ft.fields[idx].key;
                let mut current_local = 0;
                let mut target_global = None;
                for (i, field) in ft.fields.iter().enumerate() {
                    if field.key == *current_key {
                        if current_local == local_box_idx {
                            target_global = Some(i);
                            break;
                        }
                        current_local += 1;
                    }
                }
                if let Some(target_idx) = target_global {
                    self.selected_field_idx = Some(target_idx);
                    if self.is_edit_mode {
                        self.drawing_field_idx = Some(target_idx);
                    }
                    self.scroll_sidebar_to_selected(cx);
                    cx.notify();
                }
            }
        }
    }

    /// Build the ordered list of distinct field keys visible on the current page/filter.
    fn ordered_distinct_keys(&self, cx: &Context<Self>) -> Vec<String> {
        let Some(ft) = &self.form_type else {
            return Vec::new();
        };
        let filter = self.search_filter.read(cx).value().to_lowercase();
        let mut keys: Vec<String> = Vec::new();
        for field in &ft.fields {
            if field.page == self.current_page
                && (filter.is_empty() || field.key.to_lowercase().contains(&filter))
                && !keys.contains(&field.key)
            {
                keys.push(field.key.clone());
            }
        }
        keys
    }

    /// Scroll the sidebar to make the group containing `selected_field_idx` visible.
    fn scroll_sidebar_to_selected(&self, cx: &Context<Self>) {
        if let Some(ft) = &self.form_type {
            if let Some(idx) = self.selected_field_idx {
                let target_key = &ft.fields[idx].key;
                let keys = self.ordered_distinct_keys(cx);
                if let Some(group_idx) = keys.iter().position(|k| k == target_key) {
                    self.scroll_handle.scroll_to_item(group_idx);
                }
            }
        }
    }

    fn auto_pan_to_field(&mut self, idx: usize) {
        if let Some(ft) = &self.form_type {
            if let Some(f) = ft.fields.get(idx) {
                self.scale = 2.0;
                self.pan_offset = gpui::Point {
                    x: -(f.x as f32) * 2.0 + 300.0,
                    y: -(f.y as f32) * 2.0 + 300.0,
                };
            }
        }
    }

    fn add_new_field(&mut self, key: String, cx: &mut Context<Self>) {
        if let Some(ft) = &mut self.form_type {
            if let Some(first) = ft.fields.first() {
                let mut new_field = first.clone();
                new_field.key = key.clone();
                new_field.page = self.current_page;
                new_field.kind = FieldKind::Char;
                // Place roughly in the center of the viewport with a default 100x100 size
                new_field.x = ((-self.pan_offset.x + 300.0) / self.scale) as f64;
                new_field.y = ((-self.pan_offset.y + 300.0) / self.scale) as f64;
                if let Some(ref mut w) = new_field.widget {
                    w.width = 100.0;
                    w.height = 100.0;
                } else {
                    new_field.widget = Some(WidgetSpec {
                        widget_type: match new_field.kind {
                            FieldKind::Bool => WidgetType::Checkbox,
                            _ => WidgetType::Text,
                        },
                        width: 100.0,
                        height: 100.0,
                        max_length: None,
                        comb: None,
                        font_size: Some(8.5),
                    });
                }
                new_field.cell_w = None;
                ft.fields.push(new_field);
                let new_idx = ft.fields.len() - 1;
                self.selected_field_idx = Some(new_idx);
                self.drawing_field_idx = Some(new_idx);
                self.interaction_mode = InteractionMode::DrawingNew;
                self.is_edit_mode = true;
                self.scroll_sidebar_to_selected(cx);
            }
        }
    }

    fn check_collision(&self, idx: usize, field: &FormField, ft: &FormType) -> bool {
        let fx = field.x;
        let fy = field.y;
        let fw = field.display_width();
        let fh = field.display_height();

        for (i, other) in ft.fields.iter().enumerate() {
            if i == idx || other.page != self.current_page {
                continue;
            }
            let ox = other.x;
            let oy = other.y;
            let ow = other.display_width();
            let oh = other.display_height();

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
        let no_focus = self.selected_field_idx.is_none() && self.drawing_field_idx.is_none();

        let type_color = match field.kind {
            FieldKind::Char => cx.theme().warning,         // Yellowish
            FieldKind::Int => gpui::rgb(0xadb2d4).into(),  // rgb(173, 178, 212)
            FieldKind::Dec => hsla(0.38, 0.55, 0.45, 1.0), // Green
            FieldKind::Bool => gpui::rgb(0x170c79).into(), // rgb(23, 12, 121)
        };

        let (border_color, bg_color) = if is_drawing {
            // Actively editing this box — blue
            (cx.theme().info, cx.theme().info.opacity(0.4))
        } else if has_collision {
            // Overlapping another box — red
            (cx.theme().danger, cx.theme().danger.opacity(0.4))
        } else if is_selected {
            // Focused but not editing — type color
            (type_color, type_color.opacity(0.4))
        } else if no_focus {
            // No box focused — type color light
            (type_color, type_color.opacity(0.15))
        } else {
            // Default — translucent
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
            let width = field.display_width();
            let height = field.display_height();
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
                                                width: f.display_width() as f32,
                                                height: f.display_height() as f32,
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
                        // Already editing this box — start moving
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
                                        width: f.display_width() as f32,
                                        height: f.display_height() as f32,
                                    },
                                });
                            }
                        }
                    } else {
                        // Select this box
                        this.selected_field_idx = Some(idx);
                        // In edit mode, also activate editing
                        if this.is_edit_mode {
                            this.drawing_field_idx = Some(idx);
                        }
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

        let mut groups: Vec<(String, Vec<(usize, &FormField)>)> = Vec::new();
        for (idx, field) in ft.fields.iter().enumerate() {
            if field.page == self.current_page {
                if filter.is_empty() || field.key.to_lowercase().contains(&filter) {
                    if let Some(group) = groups.iter_mut().find(|(k, _)| k == &field.key) {
                        group.1.push((idx, field));
                    } else {
                        groups.push((field.key.clone(), vec![(idx, field)]));
                    }
                }
            }
        }

        let fields_list = groups
            .into_iter()
            .map(|(key, fields_in_group)| {
                let group_count = fields_in_group.len();
                let parent_key = key.clone();

                let header = div()
                    .p_2()
                    .bg(cx.theme().secondary.opacity(0.3))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(key.clone()),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    if let Some(ft) = &mut this.form_type {
                                        if let Some(last_field) = ft
                                            .fields
                                            .iter()
                                            .rev()
                                            .find(|f| f.key == parent_key)
                                            .cloned()
                                        {
                                            let mut new_field = last_field;
                                            new_field.y += new_field.size.unwrap_or(10.0) + 10.0;
                                            ft.fields.push(new_field);
                                            this.selected_field_idx = Some(ft.fields.len() - 1);
                                            this.drawing_field_idx = None;
                                            this.interaction_mode = InteractionMode::None;
                                            window.focus(&this.focus_handle, cx);
                                            cx.notify();
                                        }
                                    }
                                }),
                            )
                            .child(
                                Icon::new(IconName::Plus)
                                    .small()
                                    .text_color(cx.theme().foreground),
                            ),
                    );

                let children =
                    fields_in_group
                        .into_iter()
                        .enumerate()
                        .map(|(sub_idx, (idx, field))| {
                            let is_selected = self.selected_field_idx == Some(idx);
                            let has_collision = self.check_collision(idx, field, ft);

                            let (bg, border_style) = if is_selected {
                                (cx.theme().muted, cx.theme().primary)
                            } else if has_collision {
                                (cx.theme().danger.opacity(0.1), cx.theme().border)
                            } else {
                                (cx.theme().background, cx.theme().border)
                            };

                            let label_color = if has_collision {
                                cx.theme().danger
                            } else {
                                cx.theme().foreground
                            };

                            div()
                                .id(format!("sidebar_field_{}", idx))
                                .pl_4()
                                .pr_2()
                                .py_2()
                                .bg(bg)
                                .when(is_selected, |this| {
                                    this.border_l_4().border_color(border_style)
                                })
                                .when(!is_selected, |this| {
                                    this.border_b_1().border_color(border_style)
                                })
                                .flex()
                                .justify_between()
                                .items_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(cx.theme().muted))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _ev: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        this.selected_field_idx = Some(idx);
                                        if this.is_edit_mode {
                                            this.drawing_field_idx = Some(idx);
                                        }
                                        if let Some(ft) = &this.form_type {
                                            if let Some(_f) = ft.fields.get(idx) {
                                                this.auto_pan_to_field(idx);
                                            }
                                        }

                                        if _ev.click_count == 2 {
                                            if let Some(ft) = &this.form_type {
                                                this.char_count_modal_open = true;
                                                let initial = ft.fields[idx]
                                                    .char_count
                                                    .map(|c| c.to_string())
                                                    .unwrap_or_default();
                                                this.char_count_input.update(cx, |input, cx| {
                                                    input.set_value(initial, window, cx);
                                                    input.focus(window, cx);
                                                });
                                            }
                                        }

                                        window.focus(&this.focus_handle, cx);
                                        cx.notify();
                                    }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .w_full()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .text_color(label_color)
                                                        .text_sm()
                                                        .child(format!("{}", sub_idx + 1)),
                                                )
                                                .child(
                                                    div()
                                                        .px_1()
                                                        .py_0p5()
                                                        .rounded_sm()
                                                        .cursor_pointer()
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                            cx.stop_propagation();
                                                            this.selected_field_idx = Some(idx);
                                                            if let Some(ft) = &mut this.form_type {
                                                                let field = &mut ft.fields[idx];
                                                                field.kind = match field.kind {
                                                                    FieldKind::Char => FieldKind::Int,
                                                                    FieldKind::Int => FieldKind::Dec,
                                                                    FieldKind::Dec => FieldKind::Bool,
                                                                    FieldKind::Bool => FieldKind::Char,
                                                                };
                                                                field.direction = match field.kind {
                                                                    FieldKind::Int => Direction::Rtl,
                                                                    _ => Direction::Ltr,
                                                                };
                                                            }
                                                            cx.notify();
                                                        }))
                                                        .bg(match field.kind {
                                                            FieldKind::Char => cx.theme().warning.opacity(0.2),
                                                            FieldKind::Int => gpui::rgba(0xadb2d433).into(),
                                                            FieldKind::Dec => hsla(0.38, 0.55, 0.45, 0.2),
                                                            FieldKind::Bool => gpui::rgba(0x170c7933).into(),
                                                        })
                                                        .text_color(match field.kind {
                                                            FieldKind::Char => cx.theme().warning,
                                                            FieldKind::Int => gpui::rgb(0xadb2d4).into(),
                                                            FieldKind::Dec => hsla(0.38, 0.55, 0.45, 1.0),
                                                            FieldKind::Bool => gpui::rgb(0x170c79).into(),
                                                        })
                                                        .text_xs()
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(match field.kind {
                                                            FieldKind::Char => "CHR",
                                                            FieldKind::Int => "INT",
                                                            FieldKind::Dec => "DEC",
                                                            FieldKind::Bool => "BOL",
                                                        })
                                                )
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| {
                                                            cx.stop_propagation();
                                                            this.selected_field_idx = Some(idx);
                                                            if let Some(ft) = &mut this.form_type {
                                                                let field = &mut ft.fields[idx];
                                                                field.direction = match field.direction {
                                                                    Direction::Ltr => Direction::Rtl,
                                                                    Direction::Rtl => Direction::Ltr,
                                                                };
                                                            }
                                                            cx.notify();
                                                        }))
                                                        .text_color(cx.theme().muted_foreground)
                                                        .text_xs()
                                                        .child(match field.direction {
                                                            Direction::Ltr => "→",
                                                            Direction::Rtl => "←",
                                                        })
                                                )
                                                .child(
                                                    div()
                                                        .cursor_pointer()
                                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                                            cx.stop_propagation();
                                                            this.selected_field_idx = Some(idx);
                                                            if let Some(ft) = &this.form_type {
                                                                this.char_count_modal_open = true;
                                                                let initial = ft.fields[idx]
                                                                    .char_count
                                                                    .map(|c| c.to_string())
                                                                    .unwrap_or_default();
                                                                this.char_count_input.update(cx, |input, cx| {
                                                                    input.set_value(initial, window, cx);
                                                                    input.focus(window, cx);
                                                                });
                                                            }
                                                            cx.notify();
                                                        }))
                                                        .text_color(cx.theme().muted_foreground)
                                                        .text_xs()
                                                        .child(if let Some(cc) = field.char_count {
                                                            format!("Cc: {}", cc)
                                                        } else {
                                                            "Cc: 0".to_string()
                                                        })
                                                )
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                                    cx.stop_propagation();
                                                    if let Some(ft) = &this.form_type {
                                                        let key = ft.fields.get(idx).map(|f| f.key.clone()).unwrap_or_default();
                                                        let prompt = window.prompt(
                                                            gpui::PromptLevel::Warning,
                                                            &format!("Delete element {} from {}?", sub_idx + 1, key),
                                                            None,
                                                            &["Delete", "Cancel"],
                                                            cx,
                                                        );
                                                        cx.spawn(async move |this, cx| {
                                                            if let Ok(0) = prompt.await {
                                                                let _ = this.update(cx, |this, cx| {
                                                                    if let Some(ft) = &mut this.form_type {
                                                                        if idx < ft.fields.len() {
                                                                            ft.fields.remove(idx);
                                                                            if this.selected_field_idx == Some(idx) {
                                                                                this.selected_field_idx = None;
                                                                            } else if let Some(s) = this.selected_field_idx {
                                                                                if s > idx {
                                                                                    this.selected_field_idx = Some(s - 1);
                                                                                }
                                                                            }
                                                                            if this.drawing_field_idx == Some(idx) {
                                                                                this.drawing_field_idx = None;
                                                                            } else if let Some(d) = this.drawing_field_idx {
                                                                                if d > idx {
                                                                                    this.drawing_field_idx = Some(d - 1);
                                                                                }
                                                                            }
                                                                            cx.notify();
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        }).detach();
                                                    }
                                                }))
                                                .child(
                                                    Icon::empty()
                                                        .path("svg/minus.svg")
                                                        .small()
                                                        .text_color(cx.theme().danger)
                                                )
                                        )
                                )
                        });

                div().flex_col().child(header).children(children)
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
                    .child(Input::new(&self.search_filter)),
            )
            .child(
                div()
                    .id("fields_sidebar_list")
                    .flex_grow()
                    .min_h_0()
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
                                .child(
                                    div()
                                        .flex()
                                        .gap_1()
                                        .p_1()
                                        .bg(cx.theme().muted)
                                        .rounded_full()
                                        .child(
                                            div()
                                                .px_3()
                                                .py_1()
                                                .rounded_full()
                                                .when(!self.is_edit_mode, |this| {
                                                    this.bg(cx.theme().background)
                                                        .text_color(cx.theme().foreground)
                                                })
                                                .when(self.is_edit_mode, |this| {
                                                    this.text_color(cx.theme().muted_foreground)
                                                })
                                                .cursor_pointer()
                                                .child("View")
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _window, cx| {
                                                        this.is_edit_mode = false;
                                                        this.drawing_field_idx = None;
                                                        this.interaction_mode =
                                                            InteractionMode::None;
                                                        cx.notify();
                                                    }),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .px_3()
                                                .py_1()
                                                .rounded_full()
                                                .when(self.is_edit_mode, |this| {
                                                    this.bg(cx.theme().background)
                                                        .text_color(cx.theme().foreground)
                                                })
                                                .when(!self.is_edit_mode, |this| {
                                                    this.text_color(cx.theme().muted_foreground)
                                                })
                                                .cursor_pointer()
                                                .child("Edit")
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _window, cx| {
                                                        if !this.is_edit_mode {
                                                            this.is_edit_mode = true;
                                                            // If a field is selected, start editing it
                                                            if let Some(idx) =
                                                                this.selected_field_idx
                                                            {
                                                                this.drawing_field_idx = Some(idx);
                                                            }
                                                            this.interaction_mode =
                                                                InteractionMode::None;
                                                            cx.notify();
                                                        }
                                                    }),
                                                ),
                                        ),
                                ),
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

            // Determine which field key is focused (if any).
            // When a specific box is focused, only show boxes with that key.
            // When nothing is focused, show ALL boxes on the page.
            let focused_key = self
                .drawing_field_idx
                .or(self.selected_field_idx)
                .and_then(|idx| ft.fields.get(idx).map(|f| f.key.clone()));

            let fields_canvas = ft
                .fields
                .iter()
                .enumerate()
                .filter(|(_, f)| f.page == self.current_page)
                .filter(|(_, field)| {
                    // If a field is focused, only show boxes with that key
                    if let Some(key) = &focused_key {
                        field.key == *key
                    } else {
                        true
                    }
                })
                .map(|(idx, field)| {
                    let mut has_collision = self.check_collision(idx, field, ft);

                    // Enforce rule: Decimal fields MUST have at least 2 boxes (integer and cents)
                    if field.kind == FieldKind::Dec {
                        let total_boxes_for_key =
                            ft.fields.iter().filter(|f| f.key == field.key).count();
                        if total_boxes_for_key < 2 {
                            has_collision = true; // Mark as error (red border)
                        }
                    }

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
                    .object_fit(ObjectFit::Fill) // Force fill so SVG bounds exactly match the field container
                    .into_any_element()
            } else {
                div().bg(cx.theme().background).into_any_element()
            };

            div()
                .flex()
                .flex_row()
                .flex_grow()
                .overflow_hidden()
                .h_full()
                .child(
                    div()
                        .id("interactive_canvas_viewport")
                        .flex_grow()
                        .h_full()
                        .overflow_hidden()
                        .bg(cx.theme().muted)
                        .when(self.drawing_field_idx.is_some(), |this| {
                            this.cursor(CursorStyle::Crosshair)
                        })
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
                                                        // Only update widget dimensions — never touch cell_w or size
                                                        // (cell_w = per-char spacing for Typst, size = font size)
                                                        if let Some(ref mut ws) = f.widget {
                                                            ws.width = w as f64;
                                                            ws.height = h as f64;
                                                        } else {
                                                            f.widget = Some(WidgetSpec {
                                                                widget_type: match f.kind {
                                                                    FieldKind::Bool => {
                                                                        WidgetType::Checkbox
                                                                    }
                                                                    _ => WidgetType::Text,
                                                                },
                                                                width: w as f64,
                                                                height: h as f64,
                                                                max_length: None,
                                                                comb: None,
                                                                font_size: f.size,
                                                            });
                                                        }
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
                                                        // Only update widget dimensions — never touch cell_w or size
                                                        if let Some(ref mut ws) = f.widget {
                                                            ws.width = new_w as f64;
                                                            ws.height = new_h as f64;
                                                        } else {
                                                            f.widget = Some(WidgetSpec {
                                                                widget_type: match f.kind {
                                                                    FieldKind::Bool => {
                                                                        WidgetType::Checkbox
                                                                    }
                                                                    _ => WidgetType::Text,
                                                                },
                                                                width: new_w as f64,
                                                                height: new_h as f64,
                                                                max_length: None,
                                                                comb: None,
                                                                font_size: f.size,
                                                            });
                                                        }
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

        let mut main_container = div()
            .key_context("PdfLayoutEditorView")
            .track_focus(&self.focus_handle)
            .on_action(
                cx.listener(|this, _: &crate::global_actions::ZoomIn, _, cx| {
                    this.scale *= 1.2;
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::ZoomOut, _, cx| {
                    this.scale /= 1.2;
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::ResetZoom, _, cx| {
                    this.scale = 1.0;
                    this.pan_offset = gpui::Point::default();
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::ToggleEditMode, _, cx| {
                    if this.is_edit_mode {
                        // Exit edit mode
                        this.is_edit_mode = false;
                        this.drawing_field_idx = None;
                        this.interaction_mode = InteractionMode::None;
                    } else {
                        // Enter edit mode
                        this.is_edit_mode = true;
                        // If a field is already selected, start editing it
                        if let Some(idx) = this.selected_field_idx {
                            this.drawing_field_idx = Some(idx);
                        }
                        this.interaction_mode = InteractionMode::None;
                    }
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::SaveLayout, window, cx| {
                    this.save_file(window, cx);
                }),
            )
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorEscape, window, cx| {
                    if this.new_field_modal_open {
                        this.new_field_modal_open = false;
                    } else if this.selected_field_idx.is_some() || this.drawing_field_idx.is_some()
                    {
                        if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                            if let Some(ft) = &mut this.form_type {
                                if let Some(f) = ft.fields.get(idx) {
                                    if f.key == "__NEW_FIELD__" {
                                        ft.fields.remove(idx);
                                    }
                                }
                            }
                        }
                        this.drawing_field_idx = None;
                        this.selected_field_idx = None;
                        this.interaction_mode = InteractionMode::None;
                    } else {
                        // Second Escape: exit edit mode entirely
                        this.is_edit_mode = false;
                    }
                    window.focus(&this.focus_handle, cx);
                    cx.notify();
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorFocusSearch, window, cx| {
                    this.search_filter
                        .update(cx, |input, cx| input.focus(window, cx));
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorNewBox, _window, cx| {
                    if let Some(_ft) = &this.form_type {
                        this.add_new_field("__NEW_FIELD__".to_string(), cx);
                        cx.notify();
                    }
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorRenameField, window, cx| {
                    if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                        if let Some(ft) = &this.form_type {
                            this.new_field_modal_open = true;

                            let key = ft.fields[idx].key.clone();
                            let initial = if key == "__NEW_FIELD__" {
                                let mut prefix = String::new();
                                if let Some(first_field) = ft.fields.first() {
                                    if let Some(colon_idx) = first_field.key.find(':') {
                                        prefix = first_field.key[..=colon_idx].to_string();
                                    }
                                }
                                prefix
                            } else {
                                key
                            };

                            this.new_field_input.update(cx, |input, cx| {
                                input.set_value(initial, window, cx);
                                input.focus(window, cx);
                            });
                            cx.notify();
                        }
                    }
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorSetCharCount, window, cx| {
                    if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                        if let Some(ft) = &this.form_type {
                            this.char_count_modal_open = true;
                            let initial = ft.fields[idx]
                                .char_count
                                .map(|c| c.to_string())
                                .unwrap_or_default();

                            this.char_count_input.update(cx, |input, cx| {
                                input.set_value(initial, window, cx);
                                input.focus(window, cx);
                            });
                            cx.notify();
                        }
                    }
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorCycleType, _window, cx| {
                    if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                        if let Some(ft) = &mut this.form_type {
                            let field = &mut ft.fields[idx];
                            field.kind = match field.kind {
                                FieldKind::Char => FieldKind::Int,
                                FieldKind::Int => FieldKind::Dec,
                                FieldKind::Dec => FieldKind::Bool,
                                FieldKind::Bool => FieldKind::Char,
                            };
                            field.direction = match field.kind {
                                FieldKind::Int => Direction::Rtl,
                                _ => Direction::Ltr,
                            };
                            cx.notify();
                        }
                    }
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorToggleDirection, _window, cx| {
                    if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                        if let Some(ft) = &mut this.form_type {
                            let field = &mut ft.fields[idx];
                            field.direction = match field.direction {
                                Direction::Ltr => Direction::Rtl,
                                Direction::Rtl => Direction::Ltr,
                            };
                            cx.notify();
                        }
                    }
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorDuplicateBox, window, cx| {
                    if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                        if let Some(ft) = &mut this.form_type {
                            if let Some(field) = ft.fields.get(idx) {
                                let new_field = field.clone();
                                ft.fields.push(new_field);
                                let new_idx = ft.fields.len() - 1;
                                this.selected_field_idx = Some(new_idx);
                                this.drawing_field_idx = Some(new_idx);
                                this.interaction_mode = InteractionMode::None;
                                this.scroll_sidebar_to_selected(cx);
                                window.focus(&this.focus_handle, cx);
                                cx.notify();
                            }
                        }
                    }
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorDeleteBox, window, cx| {
                    if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                        if let Some(ft) = &this.form_type {
                            let key = ft.fields[idx].key.clone();
                            let count = ft.fields.iter().filter(|f| f.key == key).count();
                            if count <= 1 {
                                return;
                            }
                            let prompt = window.prompt(
                                gpui::PromptLevel::Warning,
                                &format!("Delete Box from {}?", key),
                                None,
                                &["Delete", "Cancel"],
                                cx,
                            );
                            cx.spawn(async move |this, cx| {
                                if let Ok(0) = prompt.await {
                                    let _ = this.update(cx, |this, cx| {
                                        if let Some(ft) = &mut this.form_type {
                                            ft.fields.remove(idx);
                                            this.selected_field_idx = None;
                                            this.drawing_field_idx = None;
                                            cx.notify();
                                        }
                                    });
                                }
                            })
                            .detach();
                        }
                    }
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorNextField, _window, cx| {
                    if let Some(ft) = &this.form_type {
                        let keys = this.ordered_distinct_keys(cx);
                        if keys.is_empty() {
                            return;
                        }
                        let mut next_key = None;

                        if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                            let current_key = ft.fields[idx].key.clone();
                            if let Some(key_pos) = keys.iter().position(|k| k == &current_key) {
                                if key_pos + 1 < keys.len() {
                                    next_key = Some(keys[key_pos + 1].clone());
                                }
                            }
                        } else {
                            next_key = Some(keys[0].clone());
                        }

                        if let Some(nk) = next_key {
                            if let Some(first_idx) = ft.fields.iter().position(|f| f.key == nk) {
                                this.selected_field_idx = Some(first_idx);
                                if this.is_edit_mode {
                                    this.drawing_field_idx = Some(first_idx);
                                }
                                this.scroll_sidebar_to_selected(cx);
                                this.auto_pan_to_field(first_idx);
                                cx.notify();
                            }
                        }
                    }
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorPrevField, _window, cx| {
                    if let Some(ft) = &this.form_type {
                        let keys = this.ordered_distinct_keys(cx);
                        if keys.is_empty() {
                            return;
                        }
                        let mut prev_key = None;

                        if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                            let current_key = ft.fields[idx].key.clone();
                            if let Some(key_pos) = keys.iter().position(|k| k == &current_key) {
                                if key_pos > 0 {
                                    prev_key = Some(keys[key_pos - 1].clone());
                                }
                            }
                        } else {
                            prev_key = Some(keys[keys.len() - 1].clone());
                        }

                        if let Some(pk) = prev_key {
                            if let Some(first_idx) = ft.fields.iter().position(|f| f.key == pk) {
                                this.selected_field_idx = Some(first_idx);
                                if this.is_edit_mode {
                                    this.drawing_field_idx = Some(first_idx);
                                }
                                this.scroll_sidebar_to_selected(cx);
                                this.auto_pan_to_field(first_idx);
                                cx.notify();
                            }
                        }
                    }
                },
            ))
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorSelectBox1, _, cx| {
                    this.focus_box(0, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorSelectBox2, _, cx| {
                    this.focus_box(1, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorSelectBox3, _, cx| {
                    this.focus_box(2, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorSelectBox4, _, cx| {
                    this.focus_box(3, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorSelectBox5, _, cx| {
                    this.focus_box(4, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorSelectBox6, _, cx| {
                    this.focus_box(5, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorSelectBox7, _, cx| {
                    this.focus_box(6, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorSelectBox8, _, cx| {
                    this.focus_box(7, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorSelectBox9, _, cx| {
                    this.focus_box(8, cx)
                }),
            )
            .on_action(cx.listener(
                |this, _: &crate::global_actions::EditorSelectLastBox, _, cx| {
                    if let Some(ft) = &this.form_type {
                        if let Some(idx) = this.selected_field_idx.or(this.drawing_field_idx) {
                            let current_key = &ft.fields[idx].key;
                            let count = ft.fields.iter().filter(|f| f.key == *current_key).count();
                            if count > 0 {
                                this.focus_box(count - 1, cx);
                            }
                        }
                    }
                },
            ))
            .size_full()
            .flex_grow()
            .overflow_hidden()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .flex()
            .flex_col()
            .child(header)
            .child(body);

        if self.new_field_modal_open {
            main_container = main_container.child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(cx.theme().background.opacity(0.8))
                    .flex()
                    .justify_center()
                    .items_center()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.new_field_modal_open = false;
                            window.focus(&this.focus_handle, cx);
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .w(px(400.))
                            .p_6()
                            .bg(cx.theme().background)
                            .rounded_xl()
                            .shadow_2xl()
                            .border_1()
                            .border_color(cx.theme().border)
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child("Add New Field"),
                            )
                            .child(
                                div()
                                    .mt_2()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Enter the field key name (e.g. frm2551Qv2018:txt1)"),
                            )
                            .child(div().mt_4().child(Input::new(&self.new_field_input))),
                    ),
            );
        }

        if self.char_count_modal_open {
            main_container = main_container.child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(cx.theme().background.opacity(0.8))
                    .flex()
                    .justify_center()
                    .items_center()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                        this.char_count_modal_open = false;
                        window.focus(&this.focus_handle, cx);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(400.))
                            .p_6()
                            .bg(cx.theme().background)
                            .rounded_xl()
                            .shadow_2xl()
                            .border_1()
                            .border_color(cx.theme().border)
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(div().text_lg().font_weight(FontWeight::BOLD).text_color(cx.theme().foreground).child("Set Char Count"))
                            .child(div().mt_2().text_sm().text_color(cx.theme().muted_foreground).child("Enter the number of characters this box can hold. Enter 0 or empty to clear."))
                            .child(div().mt_4().child(Input::new(&self.char_count_input)))
                    )
            );
        }

        main_container
    }
}
