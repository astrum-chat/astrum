use anyml::MessageRole;
use gpui::{
    App, Div, ElementId, Entity, IntoElement, Overflow, PointRefinement, SharedString, Stateful,
    Window, div, prelude::*, px,
};
use gpui_tesserae::{
    ElementIdExt,
    components::ChatBubble,
    primitives::selectable_text::{SelectableText, SelectableTextState},
    theme::ThemeExt,
};

use crate::{RgbaExt, managers::Chat};

pub fn render_existing_chat(
    base_id: &ElementId,
    current_chat: &Entity<Chat>,
    cx: &App,
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
        .children(render_messages(&current_chat.read(cx), cx))
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

fn render_messages<'a>(chat: &'a Chat, cx: &'a App) -> impl Iterator<Item = ChatMessage> + 'a {
    chat.read_messages(cx).iter().map(|(id, message)| {
        ChatMessage::new(
            id.to_string(),
            message.message.role.clone(),
            &message.message.content,
        )
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
        let selectable_content_state =
            window.use_keyed_state(self.id.with_suffix("state:content"), cx, |_window, cx| {
                SelectableTextState::new(cx)
            });

        selectable_content_state.update(cx, |this, cx| {
            if self.content == this.get_text() {
                return;
            };

            this.text(self.content.clone());
            cx.notify();
        });

        let font_family = cx.get_theme().layout.text.default_font.family[0].clone();

        let text_size = cx.get_theme().layout.text.default_font.sizes.heading_sm;
        let selection_color = cx
            .get_theme()
            .variants
            .active(cx)
            .colors
            .accent
            .primary
            .alpha(0.3);

        let selectable_content =
            SelectableText::new(self.id.with_suffix("content"), selectable_content_state)
                .selection_color(selection_color)
                .selection_rounded(px(6.))
                .selection_rounded_smoothing(1.)
                .w_auto()
                .max_w_full()
                .word_wrap(true)
                .font_family(font_family)
                .text_size(text_size);

        match self.role {
            MessageRole::User => {
                let secondary_text_color = cx.get_theme().variants.active(cx).colors.text.secondary;

                right_align(
                    ChatBubble::new("chat_bubble")
                        .child(selectable_content.text_color(secondary_text_color)),
                )
                .into_any_element()
            }
            _ => {
                let primary_text_color = cx.get_theme().variants.active(cx).colors.text.primary;

                selectable_content
                    .text_color(primary_text_color)
                    .into_any_element()
            }
        }
    }
}
