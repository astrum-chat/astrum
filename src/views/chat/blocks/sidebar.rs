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
    OpenSettings, assets::AstrumIconKind, managers::Managers, utils::search::filter_by_relevance,
};

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

        let search_chats_input_state = window.use_keyed_state(
            self.id.with_suffix("state:search_chats"),
            cx,
            |_window, cx| InputState::new(cx),
        );

        let chats = &self.managers.read_blocking().chats;
        let current_chat_id_state = chats.get_current_chat_id();
        let current_chat_id = current_chat_id_state.read(cx).as_ref();

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

        let search_query = search_chats_input_state.read(cx).value().to_string();

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
            .map(|this| match chats.chats_iter(cx) {
                Some(iter) => {
                    let filtered_chats = filter_by_relevance(iter, &search_query, |chat| {
                        chat.title.read(cx).as_str()
                    });

                    this.children(filtered_chats.into_iter().map(|chat| {
                        let current_chat_id_state = current_chat_id_state.clone();
                        let chat_id = chat.chat_id.clone();

                        Toggle::new(self.id.with_suffix(format!("thread_{}", chat_id)))
                            .text(chat.title.read(cx))
                            .variant(ToggleVariant::Secondary)
                            .checked(current_chat_id == Some(&chat_id))
                            .icon(AstrumIconKind::Chat)
                            .on_click(move |_checked, _window, cx| {
                                current_chat_id_state
                                    .update(cx, |this, _cx| *this = Some(chat_id.clone()));
                            })
                            .justify_start()
                    }))
                }

                None => this,
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
            );

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
