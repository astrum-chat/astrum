use std::sync::Arc;

use gpui::{
    App, ElementId, Fill, InteractiveElement, IntoElement, Overflow, PointRefinement, RenderOnce,
    div, prelude::*, px, relative,
};
use gpui_tesserae::{
    ElementIdExt, PositionalParentElement,
    components::{Button, ButtonVariant, Icon, Input, Toggle, ToggleVariant},
    extensions::mouse_handleable::MouseHandleable,
    primitives::input::InputState,
    theme::ThemeExt,
};
use smol::lock::RwLock;

use crate::{
    OpenSettings, PixelsExt, assets::AstrumIconKind, managers::Managers, managers::UniqueId,
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

fn collect_chat_data(chats: &crate::managers::ChatsManager, cx: &App) -> Vec<(UniqueId, String)> {
    chats
        .chats_iter(cx)
        .map(|iter| {
            iter.map(|chat| (chat.chat_id.clone(), chat.title.read(cx).clone()))
                .collect()
        })
        .unwrap_or_default()
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

impl RenderOnce for Sidebar {
    fn render(self, window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let secondary_bg_color = cx
            .get_theme()
            .variants
            .active(cx)
            .colors
            .background
            .secondary;

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
        let available_update_entity = managers.update.available_update.clone();
        let available_update = available_update_entity.read(cx).clone();
        let chats = &managers.chats;
        let current_chat_id_state = chats.get_current_chat_id();
        let current_chat_id = current_chat_id_state.read(cx).as_ref();

        let current_query = search_chats_input_state.read(cx).value().to_string();
        let search_state_data = search_state.read(cx);

        if current_query != search_state_data.last_query {
            let new_query = current_query.clone();
            let search_state = search_state.clone();
            let chat_data = collect_chat_data(chats, cx);

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

        let threads_section = div()
            .id(self.id.with_suffix("threads_section"))
            .flex()
            .flex_col()
            .p(px(10.))
            .gap(px(5.))
            .w_full()
            .h_full()
            .map(|mut this| {
                this.style().overflow = PointRefinement {
                    x: None,
                    y: Some(Overflow::Scroll),
                };
                this
            })
            .map(|this| {
                let Some(iter) = chats.chats_iter(cx) else {
                    return this.child(empty_state_text("No threads exist yet.", window, cx));
                };

                let all_chats: Vec<_> = iter.collect();
                if all_chats.is_empty() {
                    return this.child(empty_state_text("No threads exist yet.", window, cx));
                }

                let visible_chats: Vec<_> = match &filtered_ids {
                    Some(ids) => all_chats
                        .into_iter()
                        .filter(|chat| ids.contains(&chat.chat_id))
                        .collect(),
                    None => all_chats,
                };

                if visible_chats.is_empty() {
                    return this.child(empty_state_text(
                        "No threads matched this query.",
                        window,
                        cx,
                    ));
                }

                this.children(visible_chats.into_iter().map(|chat| {
                    let current_chat_id_state = current_chat_id_state.clone();
                    let chat_id = chat.chat_id.clone();

                    Toggle::new(self.id.with_suffix(format!("thread_{}", chat_id)))
                        .text(chat.title.read(cx).replace("\n", " ").replace("  ", " "))
                        .variant(ToggleVariant::Secondary)
                        .checked(current_chat_id == Some(&chat_id))
                        .icon(AstrumIconKind::Chat)
                        .on_click(move |_checked, _window, cx| {
                            current_chat_id_state
                                .update(cx, |this, _cx| *this = Some(chat_id.clone()));
                        })
                        .justify_start()
                }))
            });

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
                let available_update_entity = available_update_entity.clone();
                this.child(
                    Toggle::new(self.id.with_suffix("download_btn"))
                        .variant(ToggleVariant::Constructive)
                        .icon(AstrumIconKind::Download)
                        .icon_size(px(18.))
                        .p(px(9.))
                        .on_click(move |_event, _window, cx| {
                            let http_client = cx.http_client();
                            crate::managers::UpdateManager::apply_update(
                                http_client,
                                available_update_entity.clone(),
                                cx,
                            );
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
