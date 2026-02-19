use anyml::MessageRole;
use gpui::{
    App, Div, ElementId, Hsla, IntoElement, Overflow, PointRefinement, SharedString, Stateful,
    Styled, Window, div, prelude::*, px,
};
use gpui_tesserae::{ElementIdExt, components::ChatBubble, theme::ThemeExt};
use notitia::OrderKey;
use notitia::PrimaryKey;
use std::collections::BTreeMap;

use super::md_render::render_markdown;
use crate::{RgbaExt, managers::UniqueId};

pub fn render_existing_chat(
    base_id: &ElementId,
    messages: &BTreeMap<OrderKey, (PrimaryKey<UniqueId>, String, String)>,
) -> Stateful<Div> {
    div()
        .id(base_id.with_suffix("existing_messages"))
        .w_full()
        .h_auto()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(60.))
        .mb(px(-35.))
        .p(px(20.))
        // 20px base padding, 35px to account for margin, 175px is extra.
        .pb(px(20. + 35. + 175.))
        .map(|mut this| {
            this.style().overflow = PointRefinement {
                x: None,
                y: Some(Overflow::Scroll),
            };
            this
        })
        .children(render_messages(messages))
}

fn right_align(child: impl IntoElement) -> Div {
    div()
        .w_full()
        .h_auto()
        .flex()
        .flex_col()
        .justify_end()
        .items_end()
        .child(child)
}

fn render_messages(
    messages: &BTreeMap<OrderKey, (PrimaryKey<UniqueId>, String, String)>,
) -> impl Iterator<Item = ChatMessage> + '_ {
    messages.values().map(|(id, role, content)| {
        ChatMessage::new(id.to_string(), MessageRole::from_str(role), content)
    })
}

#[derive(IntoElement)]
struct ChatMessage {
    id: ElementId,
    role: MessageRole,
    content: SharedString,
}

impl ChatMessage {
    fn new(id: impl Into<ElementId>, role: MessageRole, content: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            role,
            content: content.into(),
        }
    }
}

impl RenderOnce for ChatMessage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let active = cx.get_theme().variants.active(cx);
        let text_color = active.colors.text.primary;
        let selection_color: Hsla = active.colors.accent.primary.alpha(0.3).into();

        let is_user = matches!(self.role, MessageRole::User);
        let bg_color: Hsla = if is_user {
            active.colors.background.quaternary
        } else {
            active.colors.background.tertiary
        }
        .into();

        let md = render_markdown(
            &self.content,
            &self.id,
            text_color,
            selection_color,
            bg_color,
            window,
            cx,
        );

        if is_user {
            right_align(ChatBubble::new("chat_bubble").child(md.max_w_full().w_auto()))
                .into_any_element()
        } else {
            md.w_full().into_any_element()
        }
    }
}
