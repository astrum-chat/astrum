use std::sync::Arc;

use gpui::{
    App, ElementId, Fill, InteractiveElement, IntoElement, RenderOnce, div, prelude::*, px,
    relative, uniform_list,
};
use gpui_tesserae::{
    ElementIdExt, PositionalParentElement,
    components::{Button, ButtonVariant, Icon, Input, Toggle, ToggleVariant},
    extensions::mouse_handleable::MouseHandleable,
    primitives::input::InputState,
    theme::ThemeExt,
};
use notitia::OrderKey;
use notitia::prelude::*;
use notitia_gpui::{DbEntity, WindowNotitiaExt};
use smol::lock::RwLock;

use crate::{
    OpenSettings, PixelsExt,
    assets::AstrumIconKind,
    managers::Managers,
    managers::UniqueId,
    schema::{AstrumDb, ChatRecord},
    utils::search::filter_by_relevance,
};

#[derive(Clone)]
struct SearchState {
    last_query: String,
    filtered_ids: Option<Vec<UniqueId>>,
}

impl SearchState {
    fn new() -> Self {
        Self {
            last_query: String::new(),
            filtered_ids: None,
        }
    }
}

#[derive(IntoElement)]
pub struct Sidebar {
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
}

impl Sidebar {
    pub fn new(id: impl Into<ElementId>, managers: Arc<RwLock<Managers>>) -> Self {
        let id = id.into();

        Self {
            id: id.clone(),
            managers,
        }
    }
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let secondary_bg_color = cx
            .get_theme()
            .variants
            .active(cx)
            .colors
            .background
            .secondary;
        let lg_size = cx.get_theme().layout.size.lg;

        let search_chats_input_state = window.use_keyed_state(
            self.id.with_suffix("state:search_chats"),
            cx,
            |_window, cx| InputState::new(cx),
        );

        let search_state = window.use_keyed_state(
            self.id.with_suffix("state:search_results"),
            cx,
            |_window, _cx| SearchState::new(),
        );

        let managers = self.managers.read_blocking();
        let available_update = managers.update.available_update.read(cx).clone();
        let chats = &managers.chats;
        let db_initialized = chats.db_initialized();
        let current_chat_id_state = chats.get_current_chat_id();
        let current_chat_id = current_chat_id_state.read(cx).clone();

        // Subscribe to the chat list via notitia (only if DB is ready)
        let chat_list: Option<DbEntity<BTreeMap<OrderKey, (UniqueId, Option<String>)>>> =
            if db_initialized {
                let db = chats.db().clone();
                Some(window.use_db_query(cx, |_window, _cx| {
                    db.query(
                        AstrumDb::CHATS
                            .select((ChatRecord::ID, ChatRecord::TITLE))
                            .order_by(ChatRecord::EDITED_AT, OrderDirection::Desc)
                            .fetch_all::<BTreeMap<_, _>>(),
                    )
                }))
            } else {
                None
            };

        let current_query = search_chats_input_state.read(cx).value().to_string();
        let search_state_data = search_state.read(cx);

        if current_query != search_state_data.last_query {
            let new_query = current_query.clone();
            let search_state = search_state.clone();

            // Collect chat data from the subscription
            let chat_data: Vec<(UniqueId, String)> = chat_list
                .as_ref()
                .and_then(|cl| cl.read(cx))
                .map(|set| {
                    set.values()
                        .map(|(id, title): &(UniqueId, Option<String>)| {
                            (
                                id.clone(),
                                title.clone().unwrap_or_else(|| "Untitled Chat".to_string()),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();

            cx.spawn(async move |cx| {
                let filtered_ids = compute_filtered_ids(chat_data, &new_query);

                let _ = search_state.update(cx, |state, cx| {
                    state.last_query = new_query;
                    state.filtered_ids = filtered_ids;
                    cx.notify();
                });
            })
            .detach();
        }

        let filtered_ids = search_state_data.filtered_ids.clone();

        let top_section = div()
            .flex()
            .flex_col()
            .pl(px(10.))
            .pr(px(10.))
            .mb(px(10.))
            .gap(px(5.))
            .w_full()
            .h_auto()
            .child(
                Input::new(
                    self.id.with_suffix("search_chats_btn"),
                    search_chats_input_state.clone(),
                )
                .placeholder("Search Chats")
                .child_left(Icon::new(AstrumIconKind::Search)),
            )
            .child(
                Button::new("new_chat_btn")
                    .text("New Chat")
                    .variant(ButtonVariant::SecondaryGhost)
                    .justify_start()
                    .child_left(Icon::new(AstrumIconKind::Plus))
                    .map(|this| {
                        let current_chat_id_state = current_chat_id_state.clone();

                        this.on_click(move |_checked, _window, cx| {
                            current_chat_id_state.update(cx, |this, _cx| *this = None);
                        })
                    }),
            );

        // Build the visible chats list from subscription data
        let visible_chats: Arc<[(UniqueId, String)]> = chat_list
            .as_ref()
            .and_then(|cl| cl.read(cx))
            .map(|set| {
                set.values()
                    .filter(
                        |(id, _title): &&(UniqueId, Option<String>)| match &filtered_ids {
                            Some(ids) => ids.contains(id),
                            None => true,
                        },
                    )
                    .map(|(id, title): &(UniqueId, Option<String>)| {
                        (
                            id.clone(),
                            title.clone().unwrap_or_else(|| "Untitled Chat".to_string()),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .into();

        let threads_section = if visible_chats.is_empty() {
            let message = if filtered_ids.is_some() {
                "No threads matched this query."
            } else {
                "No threads exist yet."
            };

            div()
                .w_full()
                .h_full()
                .px(px(10.))
                .pt(px(10.))
                .child(empty_state_text(message, window, cx))
                .into_any_element()
        } else {
            let current_id = current_chat_id.clone();
            let current_chat_id_state = current_chat_id_state.clone();
            let list_id = self.id.clone();

            uniform_list(
                self.id.with_suffix("threads_section"),
                visible_chats.len(),
                move |range, _window, cx| {
                    range
                        .map(|ix| {
                            let (chat_id, title) = &visible_chats[ix];
                            let chat_title = title.replace("\n", " ").replace("  ", " ");
                            let current_chat_id_state = current_chat_id_state.clone();
                            let chat_id_owned = chat_id.clone();

                            div().w_full().pb(px(5.)).child(
                                Toggle::new(list_id.with_suffix(format!("thread_{}", chat_id)))
                                    .text(chat_title)
                                    .variant(ToggleVariant::Secondary)
                                    .checked(current_id.as_ref() == Some(chat_id))
                                    .icon(AstrumIconKind::Chat)
                                    .on_click(move |_checked, _window, cx| {
                                        current_chat_id_state.update(cx, |this, _cx| {
                                            *this = Some(chat_id_owned.clone())
                                        });
                                    })
                                    .justify_start(),
                            )
                        })
                        .collect()
                },
            )
            .w_full()
            .h_full()
            .px(px(10.))
            .pt(px(10.))
            // All items have a bottom padding of 5px
            .pb(px(5.))
            .into_any_element()
        };

        let bottom_section = div()
            .flex()
            .flex_row()
            .p(px(10.))
            .gap(px(5.))
            .w_full()
            .h_auto()
            .child(
                Toggle::new(self.id.with_suffix("settings_btn"))
                    .variant(ToggleVariant::Tertiary)
                    .icon(AstrumIconKind::Settings)
                    .icon_size(px(18.))
                    .p(px(9.))
                    .map(|this| {
                        this.on_click(move |_event, window, cx| {
                            window.dispatch_action(Box::new(OpenSettings), cx);
                        })
                    }),
            )
            .when(available_update.is_some(), |this| {
                this.child(
                    Toggle::new(self.id.with_suffix("download_btn"))
                        .variant(ToggleVariant::Constructive)
                        .icon(AstrumIconKind::Download)
                        .icon_size(px(18.))
                        .p(px(9.))
                        .on_click(move |_event, _window, _cx| {
                            crate::managers::UpdateManager::apply_pending_update();
                        }),
                )
            });

        div()
            .id(self.id)
            .tab_group()
            .tab_index(0)
            .tab_stop(false)
            .max_w(px(300.))
            .min_w(px(300.))
            .h_full()
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .w_full()
                    .h_full()
                    .min_h_0()
                    .flex()
                    .flex_shrink()
                    .flex_col()
                    .child(top_section)
                    .child(divider(secondary_bg_color))
                    .child(threads_section),
            )
            .child(
                div()
                    .w_full()
                    .h_auto()
                    .flex()
                    .flex_col()
                    .child(divider(secondary_bg_color))
                    .child(bottom_section),
            )
    }
}

fn compute_filtered_ids(chat_data: Vec<(UniqueId, String)>, query: &str) -> Option<Vec<UniqueId>> {
    if query.is_empty() {
        return None;
    }

    let ids: Vec<UniqueId> =
        filter_by_relevance(chat_data.iter(), query, |(_id, title)| title.as_str())
            .into_iter()
            .map(|(id, _)| id.clone())
            .collect();

    Some(ids)
}

fn divider(color: impl Into<Fill>) -> impl IntoElement {
    div().w(relative(1.)).h(px(1.)).min_h(px(1.)).bg(color)
}

fn empty_state_text(message: &str, window: &gpui::Window, cx: &App) -> impl IntoElement {
    let secondary_text_color = cx.get_theme().variants.active(cx).colors.text.secondary;
    let body_size = cx.get_theme().layout.text.default_font.sizes.body;
    let line_height = cx.get_theme().layout.text.default_font.line_height;
    let vertical_padding =
        cx.get_theme()
            .layout
            .size
            .lg
            .padding_needed_for_height(window, body_size, line_height);

    div()
        .w_full()
        .flex()
        .justify_center()
        .pt(vertical_padding)
        .child(
            div()
                .text_color(secondary_text_color)
                .text_size(body_size)
                .child(message.to_string()),
        )
}
