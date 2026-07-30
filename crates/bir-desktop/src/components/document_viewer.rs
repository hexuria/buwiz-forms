use gpui::{
    AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Render,
    ScrollWheelEvent, Styled, Window, div, img, prelude::*, px,
};
use gpui_component::{ActiveTheme, Icon, IconName, Sizable};
use gpui_rsx::rsx;
use std::collections::HashMap;

pub struct InteractiveDocumentViewer {
    pub document_path: String,
    pub bboxes: HashMap<String, [u16; 4]>,
    pub active_field_id: Option<String>,

    pub scale: f32,
    pub pan_offset: gpui::Point<f32>,

    pub is_panning: bool,
    pub pan_start_mouse: Option<gpui::Point<gpui::Pixels>>,
    pub pan_start_offset: Option<gpui::Point<f32>>,

    focus_handle: FocusHandle,
}

impl InteractiveDocumentViewer {
    pub fn new(
        document_path: String,
        bboxes: HashMap<String, [u16; 4]>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            document_path,
            bboxes,
            active_field_id: None,
            scale: 1.0,
            pan_offset: gpui::Point { x: 0.0, y: 0.0 },
            is_panning: false,
            pan_start_mouse: None,
            pan_start_offset: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_active_field(&mut self, field_id: Option<String>, cx: &mut Context<Self>) {
        if self.active_field_id != field_id {
            self.active_field_id = field_id.clone();

            if let Some(id) = field_id {
                if let Some(bbox) = self.bboxes.get(&id) {
                    // Center and zoom to the bbox
                    let ymin = bbox[0] as f32 / 1000.0;
                    let xmin = bbox[1] as f32 / 1000.0;
                    let ymax = bbox[2] as f32 / 1000.0;
                    let xmax = bbox[3] as f32 / 1000.0;

                    let cx_center = xmin + (xmax - xmin) / 2.0;
                    let cy_center = ymin + (ymax - ymin) / 2.0;

                    // Fixed virtual dimensions
                    let virtual_w = 850.0;
                    let virtual_h = 1100.0;

                    let box_w = (xmax - xmin) * virtual_w;
                    let box_h = (ymax - ymin) * virtual_h;

                    let max_dim = box_w.max(box_h);

                    if max_dim > 400.0 {
                        // Large box, don't zoom much
                        self.scale = 1.0;
                        self.pan_offset = gpui::Point { x: 0.0, y: 0.0 };
                    } else {
                        // Zoom in
                        self.scale = 2.0;
                        // Center it
                        // Target pan_offset such that center of bbox is at center of view.
                        // Assuming view is roughly 400x600 for now.
                        let view_center_x = 200.0;
                        let view_center_y = 300.0;

                        self.pan_offset = gpui::Point {
                            x: view_center_x - (cx_center * virtual_w * self.scale),
                            y: view_center_y - (cy_center * virtual_h * self.scale),
                        };
                    }
                }
            }
            cx.notify();
        }
    }

    fn handle_zoom_in(&mut self, cx: &mut Context<Self>) {
        self.scale = (self.scale + 0.25).min(4.0);
        cx.notify();
    }

    fn handle_zoom_out(&mut self, cx: &mut Context<Self>) {
        self.scale = (self.scale - 0.25).max(0.5);
        cx.notify();
    }

    fn handle_zoom_fit(&mut self, cx: &mut Context<Self>) {
        self.scale = 1.0;
        self.pan_offset = gpui::Point { x: 0.0, y: 0.0 };
        cx.notify();
    }
}

impl Focusable for InteractiveDocumentViewer {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InteractiveDocumentViewer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        // 8.5 x 11 standard page aspect ratio
        let virtual_w = 850.0;
        let virtual_h = 1100.0;

        let bboxes = self.bboxes.clone();
        let active_field_id = self.active_field_id.clone();

        rsx! {
            <div
                flex_col
                w_full
                h_full
                bg={theme.background}
                border_1
                border_color={theme.border}
                rounded_lg
                overflow_hidden
            >
                // Toolbar
                <div
                    flex
                    items_center
                    justify_between
                    px_3
                    py_2
                    bg={theme.secondary.opacity(0.5)}
                    border_b_1
                    border_color={theme.border}
                >
                    <div flex items_center gap_1>
                        {div()
                            .cursor_pointer()
                            .p_1()
                            .rounded_md()
                            .hover(|s| s.bg(theme.muted))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    cx.stop_propagation();
                                    this.handle_zoom_out(cx);
                                }),
                            )
                            .child(
                                Icon::new(IconName::Minus)
                                    .small()
                                    .text_color(theme.muted_foreground),
                            )}
                        <div
                            w={px(48.0)}
                            flex
                            justify_center
                            text_xs
                            font_weight={gpui::FontWeight::BOLD}
                            text_color={theme.muted_foreground}
                        >
                            {format!("{}%", (self.scale * 100.0) as i32)}
                        </div>
                        {div()
                            .cursor_pointer()
                            .p_1()
                            .rounded_md()
                            .hover(|s| s.bg(theme.muted))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    cx.stop_propagation();
                                    this.handle_zoom_in(cx);
                                }),
                            )
                            .child(
                                Icon::new(IconName::Plus)
                                    .small()
                                    .text_color(theme.muted_foreground),
                            )}
                        <div w={px(1.0)} h={px(16.0)} bg={theme.border} mx_1 />
                        {div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .hover(|s| s.bg(theme.muted))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    cx.stop_propagation();
                                    this.handle_zoom_fit(cx);
                                }),
                            )
                            .child(
                                Icon::new(IconName::Maximize)
                                    .small()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("Fit"),
                            )}
                    </div>
                </div>
                // Canvas Area
                {div()
                    .relative()
                    .w_full()
                    .h_full()
                    .bg(theme.muted.opacity(0.3))
                    .overflow_hidden()
                    .cursor_grab()
                    .when(self.is_panning, |s| s.cursor_grabbing())
                    .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _window, cx| {
                        if ev.delta.pixel_delta(px(1.0)).y > px(0.0) {
                            this.handle_zoom_in(cx);
                        } else {
                            this.handle_zoom_out(cx);
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                            this.is_panning = true;
                            this.pan_start_mouse = Some(ev.position);
                            this.pan_start_offset = Some(this.pan_offset);
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                            this.is_panning = false;
                            cx.notify();
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                        if this.is_panning {
                            if let (Some(start_mouse), Some(start_offset)) =
                                (this.pan_start_mouse, this.pan_start_offset)
                            {
                                let dx: f32 = (ev.position.x - start_mouse.x).into();
                                let dy: f32 = (ev.position.y - start_mouse.y).into();
                                this.pan_offset = gpui::Point {
                                    x: start_offset.x + dx,
                                    y: start_offset.y + dy,
                                };
                                cx.notify();
                            }
                        }
                    }))
                    .child(
                        // Inner content that is scaled and panned
                        rsx! {
                            <div
                                absolute
                                top={px(self.pan_offset.y)}
                                left={px(self.pan_offset.x)}
                                w={px(virtual_w * self.scale)}
                                h={px(virtual_h * self.scale)}
                            >
                                {img(self.document_path.clone()).w_full().h_full()}
                                {...bboxes.into_iter().map(|(field_id, bbox)| {
                                    let ymin = bbox[0] as f32 / 1000.0;
                                    let xmin = bbox[1] as f32 / 1000.0;
                                    let ymax = bbox[2] as f32 / 1000.0;
                                    let xmax = bbox[3] as f32 / 1000.0;

                                    let is_active = Some(field_id.clone()) == active_field_id;

                                    div()
                                        .absolute()
                                        .top(gpui::relative(ymin))
                                        .left(gpui::relative(xmin))
                                        .h(gpui::relative(ymax - ymin))
                                        .w(gpui::relative(xmax - xmin))
                                        .when(is_active, |this| {
                                            this.bg(theme.info.opacity(0.2))
                                                .border_2()
                                                .border_color(theme.info)
                                        })
                                        .when(!is_active, |this| {
                                            this.border_1()
                                                .border_color(theme.primary.opacity(0.6))
                                                .hover(|s| {
                                                    s.bg(theme.primary.opacity(0.1))
                                                        .border_color(theme.primary)
                                                })
                                        })
                                })}
                            </div>
                        },
                    )}
            </div>
        }
    }
}
