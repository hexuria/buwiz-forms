use bir_core::profile::TaxpayerProfile;
use gpui::*;
use gpui_component::StyledExt;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::list::{List, ListDelegate, ListEvent, ListItem, ListState};
use gpui_component::{ActiveTheme, Icon, IconName, IndexPath, h_flex, v_flex};

pub enum CommandPaletteEvent {
    SelectProfile(String),
    CreateProfile(String),
    EditProfile(String),
    Dismiss,
}

pub struct ProfileListDelegate {
    pub all_items: Vec<TaxpayerProfile>,
    pub active_items: Vec<TaxpayerProfile>,
    pub archived_items: Vec<TaxpayerProfile>,
    pub selected_index: Option<IndexPath>,
    pub create_query: Option<String>,
    pub hide_tax_profiles: bool,
}

impl ListDelegate for ProfileListDelegate {
    type Item = ListItem;

    fn sections_count(&self, _cx: &App) -> usize {
        3
    }

    fn items_count(&self, section: usize, _cx: &App) -> usize {
        match section {
            0 => self.active_items.len(),
            1 => self.archived_items.len(),
            2
                if self.create_query.is_some() => {
                    1
                }
            _ => 0,
        }
    }

    fn render_section_header(
        &mut self,
        section: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        if section == 0 && self.active_items.is_empty() {
            return None;
        }
        if section == 1 && self.archived_items.is_empty() {
            return None;
        }
        if section == 2 {
            return None;
        } // No header for Create action

        let title = if section == 0 {
            "ACTIVE PROFILES"
        } else {
            "ARCHIVED PROFILES"
        };

        Some(
            div()
                .px_4()
                .py_2()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let selected = Some(ix) == self.selected_index;
        let bg_color = if selected {
            cx.theme().secondary // hover/selected effect
        } else {
            gpui::rgba(0x00000000).into()
        };

        if ix.section == 2
            && let Some(query) = &self.create_query {
                return Some(
                    ListItem::new(ix)
                        .bg(bg_color)
                        .rounded_lg()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_4()
                                .px_4()
                                .py_3()
                                .child(
                                    Icon::new(IconName::Plus)
                                        .text_color(if selected {
                                            cx.theme().foreground
                                        } else {
                                            cx.theme().muted_foreground
                                        })
                                        .size(px(24.)),
                                )
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(cx.theme().foreground)
                                        .child(format!("Create new profile for \"{}\"", query)),
                                ),
                        )
                        .selected(selected),
                );
            }

        let profile = if ix.section == 0 {
            self.active_items.get(ix.row)
        } else {
            self.archived_items.get(ix.row)
        };

        profile.map(|p| {
            ListItem::new(ix)
                .bg(bg_color)
                .rounded_lg()
                .child(
                    h_flex()
                        .items_center()
                        .gap_4()
                        .px_4()
                        .py_3()
                        .child(
                            Icon::new(IconName::User)
                                .text_color(if selected {
                                    cx.theme().foreground
                                } else {
                                    cx.theme().muted_foreground
                                })
                                .size(px(24.)),
                        )
                        .child(
                            v_flex()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(if p.is_archived {
                                            cx.theme().muted_foreground
                                        } else {
                                            cx.theme().foreground
                                        })
                                        .child(p.full_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("TIN: {}", p.tin.full())),
                                ),
                        ),
                )
                .selected(selected)
        })
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        let q = query.to_lowercase();
        let filtered: Vec<TaxpayerProfile> = if self.hide_tax_profiles {
            if q.is_empty() {
                Vec::new()
            } else {
                self.all_items
                    .iter()
                    .filter(|p| {
                        p.tin.full() == q.trim() || p.tin.formatted().to_lowercase() == q.trim()
                    })
                    .cloned()
                    .collect()
            }
        } else {
            self.all_items
                .iter()
                .filter(|p| {
                    q.is_empty()
                        || p.full_name.to_lowercase().contains(&q)
                        || p.tin.full().contains(&q)
                        || p.tin.formatted().to_lowercase().contains(&q)
                })
                .take(5) // Limit to 5 max
                .cloned()
                .collect()
        };

        self.active_items = filtered
            .iter()
            .filter(|p| !p.is_archived)
            .cloned()
            .collect();
        self.archived_items = filtered.iter().filter(|p| p.is_archived).cloned().collect();

        let exact_tin_match = self.all_items.iter().any(|p| {
            p.tin.full() == q.trim() || p.tin.formatted().to_lowercase() == q.trim()
        });

        if !query.trim().is_empty()
            && !exact_tin_match
            && !self
                .all_items
                .iter()
                .any(|p| p.full_name.to_lowercase() == q.trim())
        {
            self.create_query = Some(query.trim().to_string());
        } else {
            self.create_query = None;
        }

        Task::ready(())
    }
}

pub struct CommandPalette {
    input_state: Entity<InputState>,
    list_state: Entity<ListState<ProfileListDelegate>>,
}

impl CommandPalette {
    pub fn new(
        profiles: Vec<TaxpayerProfile>,
        hide_tax_profiles: bool,
        active_session_tin: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filtered: Vec<TaxpayerProfile> = if hide_tax_profiles {
            // Show only the active session profile if one exists
            if let Some(ref session_tin) = active_session_tin {
                profiles
                    .iter()
                    .filter(|p| p.tin.full() == *session_tin)
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            profiles.clone().into_iter().take(5).collect()
        };
        let active_items: Vec<TaxpayerProfile> = filtered
            .iter()
            .filter(|p| !p.is_archived)
            .cloned()
            .collect();
        let archived_items: Vec<TaxpayerProfile> =
            filtered.iter().filter(|p| p.is_archived).cloned().collect();

        let initial_selection = if !active_items.is_empty() {
            Some(IndexPath::new(0).section(0))
        } else if !archived_items.is_empty() {
            Some(IndexPath::new(0).section(1))
        } else {
            None
        };

        let delegate = ProfileListDelegate {
            all_items: profiles.clone(),
            active_items,
            archived_items,
            selected_index: initial_selection,
            create_query: None,
            hide_tax_profiles,
        };

        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        let input_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search profiles..."));

        cx.subscribe_in(
            &input_state,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| {
                match event {
                    InputEvent::Change => {
                        let query = this.input_state.read(cx).value().to_string();
                        this.list_state.update(cx, |list, cx| {
                            list.delegate_mut()
                                .perform_search(&query, window, cx)
                                .detach();

                            // Reselect first available
                            let active_count = list.delegate().active_items.len();
                            let archived_count = list.delegate().archived_items.len();
                            let create_count = if list.delegate().create_query.is_some() {
                                1
                            } else {
                                0
                            };
                            if active_count > 0 {
                                list.set_selected_index(
                                    Some(IndexPath::new(0).section(0)),
                                    window,
                                    cx,
                                );
                            } else if archived_count > 0 {
                                list.set_selected_index(
                                    Some(IndexPath::new(0).section(1)),
                                    window,
                                    cx,
                                );
                            } else if create_count > 0 {
                                list.set_selected_index(
                                    Some(IndexPath::new(0).section(2)),
                                    window,
                                    cx,
                                );
                            } else {
                                list.set_selected_index(None, window, cx);
                            }
                            cx.notify();
                        });
                    }
                    InputEvent::PressEnter { .. } => {
                        let ix = this.list_state.read(cx).delegate().selected_index;
                        if let Some(ix) = ix {
                            if ix.section == 2 {
                                if let Some(query) =
                                    &this.list_state.read(cx).delegate().create_query
                                {
                                    cx.emit(CommandPaletteEvent::CreateProfile(query.clone()));
                                }
                            } else {
                                let profile = if ix.section == 0 {
                                    this.list_state.read(cx).delegate().active_items.get(ix.row)
                                } else {
                                    this.list_state
                                        .read(cx)
                                        .delegate()
                                        .archived_items
                                        .get(ix.row)
                                };
                                if let Some(tin) = profile.map(|p| p.tin.full()) {
                                    if window.modifiers().platform || window.modifiers().control {
                                        cx.emit(CommandPaletteEvent::EditProfile(tin));
                                    } else {
                                        cx.emit(CommandPaletteEvent::SelectProfile(tin));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            },
        )
        .detach();

        cx.subscribe(
            &list_state,
            |this: &mut Self, _, event: &ListEvent, cx| match event {
                ListEvent::Confirm(ix) | ListEvent::Select(ix) => {
                    let ix = *ix;
                    if ix.section == 2 {
                        if let Some(query) = &this.list_state.read(cx).delegate().create_query {
                            cx.emit(CommandPaletteEvent::CreateProfile(query.clone()));
                        }
                    } else {
                        let profile = if ix.section == 0 {
                            this.list_state.read(cx).delegate().active_items.get(ix.row)
                        } else {
                            this.list_state
                                .read(cx)
                                .delegate()
                                .archived_items
                                .get(ix.row)
                        };
                        if let Some(tin) = profile.map(|p| p.tin.full()) {
                            cx.emit(CommandPaletteEvent::SelectProfile(tin));
                        }
                    }
                }
                ListEvent::Cancel => cx.emit(CommandPaletteEvent::Dismiss),
            },
        )
        .detach();

        Self {
            input_state,
            list_state,
        }
    }

    pub fn focus_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.focus_view(&self.input_state, window);
    }
}

impl EventEmitter<CommandPaletteEvent> for CommandPalette {}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000080))
            .flex()
            .justify_center()
            .items_start()
            .pt_32()
            .child(
                v_flex()
                    .on_mouse_down_out(cx.listener(|_, _, _, cx| {
                        cx.emit(CommandPaletteEvent::Dismiss);
                    }))
                    .w(px(600.))
                    .bg(cx.theme().background)
                    .rounded_xl()
                    .border_1()
                    .border_color(cx.theme().border)
                    .shadow_2xl()
                    .overflow_hidden()
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                        match ev.keystroke.key.as_str() {
                            "escape" => cx.emit(CommandPaletteEvent::Dismiss),
                            "up" => {
                                let ix = this
                                    .list_state
                                    .read(cx)
                                    .delegate()
                                    .selected_index
                                    .unwrap_or(IndexPath::new(0).section(0));
                                let mut new_section = ix.section;
                                let mut new_row = ix.row;

                                if new_row > 0 {
                                    new_row -= 1;
                                } else if new_section == 2 {
                                    let count_1 =
                                        this.list_state.read(cx).delegate().archived_items.len();
                                    let count_0 =
                                        this.list_state.read(cx).delegate().active_items.len();
                                    if count_1 > 0 {
                                        new_section = 1;
                                        new_row = count_1 - 1;
                                    } else if count_0 > 0 {
                                        new_section = 0;
                                        new_row = count_0 - 1;
                                    }
                                } else if new_section == 1 {
                                    let count_0 =
                                        this.list_state.read(cx).delegate().active_items.len();
                                    if count_0 > 0 {
                                        new_section = 0;
                                        new_row = count_0 - 1;
                                    }
                                }
                                this.list_state.update(cx, |list, cx| {
                                    list.set_selected_index(
                                        Some(IndexPath::new(new_row).section(new_section)),
                                        window,
                                        cx,
                                    );
                                    list.scroll_to_selected_item(window, cx);
                                });
                            }
                            "down" => {
                                let ix = this
                                    .list_state
                                    .read(cx)
                                    .delegate()
                                    .selected_index
                                    .unwrap_or(IndexPath::new(0).section(0));
                                let count_0 =
                                    this.list_state.read(cx).delegate().active_items.len();
                                let count_1 =
                                    this.list_state.read(cx).delegate().archived_items.len();
                                let count_2 =
                                    if this.list_state.read(cx).delegate().create_query.is_some() {
                                        1
                                    } else {
                                        0
                                    };

                                let mut new_section = ix.section;
                                let mut new_row = ix.row;

                                if ix.section == 0 {
                                    if ix.row + 1 < count_0 {
                                        new_row += 1;
                                    } else if count_1 > 0 {
                                        new_section = 1;
                                        new_row = 0;
                                    } else if count_2 > 0 {
                                        new_section = 2;
                                        new_row = 0;
                                    }
                                } else if ix.section == 1 {
                                    if ix.row + 1 < count_1 {
                                        new_row += 1;
                                    } else if count_2 > 0 {
                                        new_section = 2;
                                        new_row = 0;
                                    }
                                }
                                this.list_state.update(cx, |list, cx| {
                                    list.set_selected_index(
                                        Some(IndexPath::new(new_row).section(new_section)),
                                        window,
                                        cx,
                                    );
                                    list.scroll_to_selected_item(window, cx);
                                });
                            }
                            _ => {}
                        }
                    }))
                    .child(
                        h_flex()
                            .px_6()
                            .py_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .child(
                                div().w_full().text_xl().child(
                                    Input::new(&self.input_state).appearance(false).w_full(),
                                ),
                            )
                            .child(Button::new("close").ghost().icon(IconName::Close).on_click(
                                cx.listener(|_, _, _, cx| cx.emit(CommandPaletteEvent::Dismiss)),
                            )),
                    )
                    .child(
                        v_flex().w_full().h(px(350.)).pt_2().child(
                            List::new(&self.list_state)
                                .w_full()
                                .h_full()
                                .paddings(gpui::Edges::all(px(8.))),
                        ),
                    ),
            )
    }
}
