use crate::platform;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::*;

/// Event emitted when the user records a new hotkey or clears it.
pub enum HotkeyRecorderEvent {
    /// The user pressed a valid combo. The `String` is the stored combo
    /// (e.g. "control+alt+F12", "alt+F12").
    Changed(String),
    /// The user cleared the hotkey binding.
    Cleared,
}

/// A "press to record" hotkey capture widget.
///
/// Displays the current binding as styled key pills with platform-appropriate
/// modifiers (e.g. `Ctrl + Option + E` on macOS, `Win + Shift + E` on Windows).
/// Click to enter recording mode — the next qualifying keypress becomes the new binding.
/// Supports an empty/unset state when the user doesn't want a global hotkey.
pub struct HotkeyRecorder {
    /// The currently bound combo (e.g. "control+alt+F12"), or None if unset.
    current_key: Option<String>,
    /// Whether we're waiting for the user to press a key.
    recording: bool,
    /// Transient hint shown while recording (e.g. "hold a modifier").
    recording_hint: Option<String>,
    focus_handle: FocusHandle,
}

impl EventEmitter<HotkeyRecorderEvent> for HotkeyRecorder {}

impl HotkeyRecorder {
    pub fn new(
        initial_key: Option<String>,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        Self {
            current_key: initial_key.filter(|k| !k.is_empty()),
            recording: false,
            recording_hint: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Update the displayed combo externally (e.g. after loading from DB).
    pub fn set_key(&mut self, key: Option<String>, _cx: &mut Context<'_, Self>) {
        self.current_key = key.filter(|k| !k.is_empty());
    }

    pub fn current_key(&self) -> Option<&str> {
        self.current_key.as_deref()
    }

    fn start_recording(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.recording = true;
        self.recording_hint = None;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn cancel_recording(&mut self, cx: &mut Context<'_, Self>) {
        self.recording = false;
        self.recording_hint = None;
        cx.notify();
    }

    fn accept_combo(&mut self, combo: String, cx: &mut Context<'_, Self>) {
        self.current_key = Some(combo.clone());
        self.recording = false;
        self.recording_hint = None;
        cx.emit(HotkeyRecorderEvent::Changed(combo));
        cx.notify();
    }

    fn clear_key(&mut self, cx: &mut Context<'_, Self>) {
        self.current_key = None;
        self.recording = false;
        self.recording_hint = None;
        cx.emit(HotkeyRecorderEvent::Cleared);
        cx.notify();
    }

    /// Returns true if the key string is a valid bindable key (single letter, digit, or F-key).
    fn is_valid_key(key: &str) -> bool {
        let k = key.to_uppercase();
        // Single letter A-Z or digit 0-9
        if k.len() == 1 {
            let c = k.chars().next().unwrap();
            return c.is_ascii_uppercase() || c.is_ascii_digit();
        }
        // F1-F12
        if let Some(stripped) = k.strip_prefix('F') {
            if let Ok(n) = stripped.parse::<u8>() {
                return (1..=12).contains(&n);
            }
        }
        false
    }

    /// Render a single styled key pill.
    fn key_pill(label: &str, cx: &Context<'_, Self>) -> Div {
        div()
            .px_1p5()
            .py_0p5()
            .rounded(px(4.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(cx.theme().foreground)
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .min_w(px(22.0))
            .child(label.to_string())
    }

    /// Render a small "+" separator between pills.
    fn plus_separator(cx: &Context<'_, Self>) -> Div {
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child("+")
    }
}

impl Render for HotkeyRecorder {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let border_color = if self.recording {
            cx.theme().accent
        } else {
            cx.theme().border
        };

        let bg = if self.recording {
            cx.theme().accent.opacity(0.05)
        } else {
            cx.theme().background
        };

        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("hotkey_recorder")
                    .track_focus(&self.focus_handle)
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_3()
                    .py_1p5()
                    .rounded_lg()
                    .border_1()
                    .border_color(border_color)
                    .bg(bg)
                    .when(self.recording, |el| el.border_2())
                    .on_click(cx.listener(|this, _ev: &ClickEvent, window, cx| {
                        if this.recording {
                            this.cancel_recording(cx);
                        } else {
                            this.start_recording(window, cx);
                        }
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                        if !this.recording {
                            return;
                        }

                        let key_str = event.keystroke.key.as_str();

                        // Ignore bare modifier presses
                        if matches!(
                            key_str,
                            "shift" | "control" | "alt" | "meta" | "super" | "cmd"
                        ) {
                            return;
                        }

                        // Escape cancels
                        if key_str == "escape" {
                            this.cancel_recording(cx);
                            return;
                        }

                        // Backspace/Delete clears the binding
                        if matches!(key_str, "backspace" | "delete") {
                            this.clear_key(cx);
                            return;
                        }

                        let upper = key_str.to_uppercase();
                        if !Self::is_valid_key(&upper) {
                            return;
                        }

                        // Capture the exact modifiers the user is holding. A
                        // global hotkey needs at least one modifier, otherwise
                        // it would hijack the bare key system-wide.
                        let mods = &event.keystroke.modifiers;
                        match crate::platform::build_hotkey_combo(
                            mods.control,
                            mods.alt,
                            mods.shift,
                            mods.platform,
                            &upper,
                        ) {
                            Some(combo) => this.accept_combo(combo, cx),
                            None => {
                                this.recording_hint = Some(
                                    "Hold at least one modifier (e.g. Ctrl / Option) with the key."
                                        .to_string(),
                                );
                                cx.notify();
                            }
                        }
                    }))
                    .child(if self.recording {
                        // Recording mode: pulsing red dot + prompt
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(cx.theme().danger)
                                    .with_animation(
                                        "recording_pulse",
                                        Animation::new(std::time::Duration::from_millis(1000))
                                            .repeat()
                                            .with_easing(bounce(ease_in_out)),
                                        |el, delta| el.opacity(0.3 + delta * 0.7),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(if self.recording_hint.is_some() {
                                        cx.theme().danger
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .child(self.recording_hint.clone().unwrap_or_else(|| {
                                        "Press modifiers + a key… (Backspace to clear)".to_string()
                                    })),
                            )
                    } else if let Some(ref combo) = self.current_key {
                        // Display mode: parse the stored combo into modifier
                        // pills + key pill (shows the actual captured combo).
                        let (labels, key) = platform::parse_hotkey_display(combo)
                            .unwrap_or_else(|| (Vec::new(), combo.clone()));
                        let mut row = div().flex().items_center().gap_1();

                        for (i, label) in labels.iter().enumerate() {
                            if i > 0 {
                                row = row.child(Self::plus_separator(cx));
                            }
                            row = row.child(Self::key_pill(label, cx));
                        }

                        // Final separator + the actual bound key (highlighted with accent)
                        if !labels.is_empty() {
                            row = row.child(Self::plus_separator(cx));
                        }
                        row = row.child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded(px(4.0))
                                .border_1()
                                .border_color(cx.theme().primary.opacity(0.5))
                                .bg(cx.theme().primary.opacity(0.1))
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().primary)
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .min_w(px(22.0))
                                .child(key),
                        );

                        row
                    } else {
                        // Empty state: no hotkey bound
                        div().flex().items_center().child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Not set — click to assign"),
                        )
                    }),
            )
            // Clear button (only visible when a key is bound and not recording)
            .when(self.current_key.is_some() && !self.recording, |el| {
                el.child(
                    div()
                        .id("hotkey_clear_btn")
                        .cursor_pointer()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .hover(|s| s.text_color(cx.theme().danger))
                        .child("Clear")
                        .on_click(cx.listener(|this, _ev: &ClickEvent, _window, cx| {
                            this.clear_key(cx);
                        })),
                )
            })
    }
}
