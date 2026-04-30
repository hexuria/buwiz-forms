#![allow(dead_code)]
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use gpui::*;
use gpui_component::ActiveTheme;

use crate::components::combobox::{Combobox, ComboboxEvent, ComboboxState};

#[derive(PartialEq)]
enum InteractionMode {
    None,
    PanningBoth,
    PanningTypstOnly,
    AdjustingOpacity,
}

pub struct TypstCalibrationView {
    focus_handle: FocusHandle,
    formtypes_dir: PathBuf,
    _available_forms: Vec<String>,
    pub form_select: Entity<ComboboxState>,

    selected_form_id: Option<String>,
    current_page: usize,

    // Layout
    scale: f32,
    pan_offset: gpui::Point<f32>,
    typst_pan_offset: gpui::Point<f32>, // Independent offset for the typst overlay

    // Controls
    opacity: f32,
    _invert_typst: bool,
    interaction_mode: InteractionMode,
    pan_start_offset: Option<gpui::Point<f32>>,
    typst_pan_start_offset: Option<gpui::Point<f32>>,

    // Background watcher
    _timer: Option<Task<()>>,
    last_modified: Option<SystemTime>,
    force_rebuild_ticker: usize, // used to bust cache on images
    last_mouse_pos: Option<gpui::Point<f32>>,
}

impl TypstCalibrationView {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let formtypes_dir = crate::platform::find_resource_dir("formtypes");
        let available_forms = Self::discover_forms(&formtypes_dir);

        let form_select = cx.new(|cx| ComboboxState::new(available_forms.clone(), window, cx));

        cx.subscribe(
            &form_select,
            |this: &mut Self, _state, event: &ComboboxEvent, cx| {
                if let Some(form_id) = &event.selected {
                    this.load_form(form_id.clone(), cx);
                }
            },
        )
        .detach();

        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            formtypes_dir,
            _available_forms: available_forms,
            form_select,
            selected_form_id: None,
            current_page: 1,
            scale: 1.0,
            pan_offset: gpui::Point::default(),
            typst_pan_offset: gpui::Point::default(),
            opacity: 0.5,
            _invert_typst: false,
            interaction_mode: InteractionMode::None,
            pan_start_offset: None,
            typst_pan_start_offset: None,
            _timer: None,
            last_modified: None,
            force_rebuild_ticker: 0,
            last_mouse_pos: None,
        }
    }

    fn discover_forms(formtypes_dir: &Path) -> Vec<String> {
        let mut forms = Vec::new();
        if let Ok(entries) = std::fs::read_dir(formtypes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("calibration.typ").exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        forms.push(name.to_string());
                    }
                }
            }
        }
        forms.sort();
        forms
    }

    pub fn load_form(&mut self, form_id: String, cx: &mut Context<Self>) {
        self.selected_form_id = Some(form_id);
        self.current_page = 1;
        self.scale = 1.0;
        self.pan_offset = gpui::Point::default();
        self.typst_pan_offset = gpui::Point::default();
        self.opacity = 0.5;
        self.last_modified = None;

        self.start_watcher(cx);
        self.recompile_typst(cx); // initial compile
        cx.notify();
    }

    fn start_watcher(&mut self, cx: &mut Context<Self>) {
        let typ_path = self
            .formtypes_dir
            .join(self.selected_form_id.as_ref().unwrap())
            .join("calibration.typ");

        let timer = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if let Ok(metadata) = std::fs::metadata(&typ_path) {
                    if let Ok(modified) = metadata.modified() {
                        let mut should_recompile = false;
                        let _ = this.update(cx, |this: &mut TypstCalibrationView, _cx| {
                            if this.last_modified != Some(modified) {
                                this.last_modified = Some(modified);
                                should_recompile = true;
                            }
                        });

                        if should_recompile {
                            let _ = this.update(cx, |this, cx| {
                                this.recompile_typst(cx);
                            });
                        }
                    }
                }
            }
        });
        self._timer = Some(timer);
    }

    fn recompile_typst(&mut self, cx: &mut Context<Self>) {
        if let Some(form_id) = &self.selected_form_id {
            let form_dir = self.formtypes_dir.join(form_id);
            let typ_path = form_dir.join("calibration.typ");

            // Compile to PNG (preview.png, preview-1.png depending on pages)
            let _ = Command::new("typst")
                .arg("compile")
                .arg("--format")
                .arg("png")
                .arg("--ppi")
                .arg("144")
                .arg(&typ_path)
                .arg(form_dir.join("preview-{n}.png"))
                .output();

            self.force_rebuild_ticker += 1;
            cx.notify();
        }
    }
}

impl Render for TypstCalibrationView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = self.scale;
        let pan = self.pan_offset;
        let t_pan = self.typst_pan_offset;

        let canvas = if let Some(form_id) = &self.selected_form_id {
            let form_dir = self.formtypes_dir.join(form_id);

            // SVG base
            let svg_path = form_dir
                .join("pages")
                .join(format!("page{}.svg", self.current_page));
            let svg_el = if svg_path.exists() {
                img(svg_path)
                    .size_full()
                    .object_fit(ObjectFit::Fill)
                    .into_any_element()
            } else {
                div()
                    .bg(cx.theme().background)
                    .child("Missing SVG Background")
                    .into_any_element()
            };

            // Typst PNG overlay
            let png_path = form_dir.join(format!("preview-{}.png", self.current_page));
            let mut overlay_el = div().absolute().top_0().left_0().size_full();

            if png_path.exists() {
                // To bust GPUI image cache on hot-reload, we might need to load it dynamically or rely on it just working.
                // Using a unique path string normally forces reload, but `img()` takes a path.
                // We will read it as bytes to force hot reload.
                if png_path.exists() {
                    overlay_el =
                        overlay_el.child(img(png_path).size_full().object_fit(ObjectFit::Fill));
                }
            }

            // Invert logic (Difference Mode) via simple CSS if possible, but actually we will just set opacity
            overlay_el = overlay_el.opacity(self.opacity);

            div()
                .id("interactive_canvas_viewport")
                .flex_grow()
                .h_full()
                .overflow_hidden()
                .bg(cx.theme().muted)
                .on_scroll_wheel(
                    cx.listener(move |this, ev: &ScrollWheelEvent, _window, cx| {
                        if this.interaction_mode != InteractionMode::None {
                            return;
                        }

                        if ev.modifiers.shift || ev.modifiers.control || ev.modifiers.alt {
                            let delta = f32::from(ev.delta.pixel_delta(Pixels::from(100.0)).y);
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
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(pan.y))
                        .left(px(pan.x))
                        .w(px(612.0 * scale))
                        .h(px(936.0 * scale))
                        .border_1()
                        .border_color(cx.theme().border)
                        .shadow_sm()
                        .child(div().absolute().top_0().left_0().size_full().child(svg_el))
                        .child(
                            div()
                                .absolute()
                                .top(px(t_pan.y))
                                .left(px(t_pan.x))
                                .size_full()
                                .child(overlay_el),
                        ),
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(cx.theme().muted_foreground)
                .child("Select a form with a calibration.typ file to begin calibration.")
                .into_any_element()
        };

        div()
            .key_context("TypstCalibrationView")
            .track_focus(&self.focus_handle)
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorNextField, _, cx| {
                    this.typst_pan_offset.x += 10.0;
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::EditorPrevField, _, cx| {
                    this.typst_pan_offset.x -= 10.0;
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::OpacityIncrease, _, cx| {
                    this.opacity = (this.opacity + 0.1).clamp(0.0, 1.0);
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::OpacityDecrease, _, cx| {
                    this.opacity = (this.opacity - 0.1).clamp(0.0, 1.0);
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::NextPage, _, cx| {
                    if let Some(form_id) = &this.selected_form_id {
                        let next_page = this.current_page + 1;
                        let svg_path = this.formtypes_dir.join(form_id).join("pages").join(format!("page{next_page}.svg"));
                        if svg_path.exists() {
                            this.current_page = next_page;
                            cx.notify();
                        }
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::PrevPage, _, cx| {
                    if this.current_page > 1 {
                        this.current_page -= 1;
                        cx.notify();
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::ZoomIn, _, cx| {
                    this.scale = (this.scale * 1.1).clamp(0.1, 10.0);
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::ZoomOut, _, cx| {
                    this.scale = (this.scale * 0.9).clamp(0.1, 10.0);
                    cx.notify();
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_actions::ResetZoom, _, cx| {
                    this.scale = 1.0;
                    this.pan_offset = gpui::Point::default();
                    this.typst_pan_offset = gpui::Point::default();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                    if ev.modifiers.alt {
                        this.interaction_mode = InteractionMode::PanningTypstOnly;
                        this.typst_pan_start_offset = Some(gpui::Point {
                            x: f32::from(ev.position.x) - this.typst_pan_offset.x,
                            y: f32::from(ev.position.y) - this.typst_pan_offset.y,
                        });
                    } else if ev.modifiers.platform || ev.modifiers.control {
                        this.interaction_mode = InteractionMode::PanningBoth;
                        this.pan_start_offset = Some(gpui::Point {
                            x: f32::from(ev.position.x) - this.pan_offset.x,
                            y: f32::from(ev.position.y) - this.pan_offset.y,
                        });
                    }
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                    this.interaction_mode = InteractionMode::None;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                let current_pos = gpui::Point {
                    x: f32::from(ev.position.x),
                    y: f32::from(ev.position.y),
                };

                let delta_x = if let Some(last) = this.last_mouse_pos {
                    current_pos.x - last.x
                } else {
                    0.0
                };
                this.last_mouse_pos = Some(current_pos);

                match this.interaction_mode {
                    InteractionMode::PanningBoth => {
                        if let Some(start) = this.pan_start_offset {
                            this.pan_offset = gpui::Point {
                                x: current_pos.x - start.x,
                                y: current_pos.y - start.y,
                            };
                            cx.notify();
                        }
                    }
                    InteractionMode::PanningTypstOnly => {
                        if let Some(start) = this.typst_pan_start_offset {
                            this.typst_pan_offset = gpui::Point {
                                x: current_pos.x - start.x,
                                y: current_pos.y - start.y,
                            };
                            cx.notify();
                        }
                    }
                    InteractionMode::AdjustingOpacity => {
                        this.opacity = (this.opacity + delta_x * 0.005).clamp(0.0, 1.0);
                        cx.notify();
                    }
                    _ => {}
                }
            }))
            .size_full()
            .flex()
            .flex_row()
            .child(canvas)
            .child(
                div()
                    .w(px(300.0))
                    .h_full()
                    .border_l_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .child("Calibration Tool"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Onion-Skinning Editor"),
                            ),
                    )
                    .child(
                        div()
                            .p_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(div().mb_2().text_sm().child("Target Form"))
                            .child(Combobox::new(&self.form_select)),
                    )
                    .child(
                        div()
                            .p_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(div().text_sm().child("Opacity Layer (Typst)"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_family("Courier")
                                            .text_color(cx.theme().primary)
                                            .child(format!("{:.0}%", self.opacity * 100.0)),
                                    ),
                            )
                            .child(
                                div()
                                    .id("opacity-slider-track")
                                    .h_6()
                                    .w_full()
                                    .cursor_ew_resize()
                                    .flex()
                                    .items_center()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                                            this.interaction_mode =
                                                InteractionMode::AdjustingOpacity;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .relative()
                                            .h_2()
                                            .w_full()
                                            .bg(cx.theme().border)
                                            .rounded_full()
                                            .child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .left_0()
                                                    .h_full()
                                                    .w(px(268.0 * self.opacity)) // Sidebar is 300px, padding is 16px*2 = 32px. Width = 268px.
                                                    .bg(cx.theme().primary)
                                                    .rounded_full(),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .mt_4()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Hotkeys:"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("• Cmd/Ctrl + Left/Right = Prev/Next Page"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("• Cmd/Ctrl + ,/. = Opacity -/+"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("• Cmd/Ctrl + Left Drag = Pan Both"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("• Alt + Left Drag = Pan Typst Only"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("• Cmd/Ctrl + Scroll = Zoom"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("• Cmd + [/] = Gap +/-"),
                            ),
                    ),
            )
    }
}
