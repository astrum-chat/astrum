use anyml::{Message, MessageRole};
use gpui::{
    AnyElement, App, Div, ElementId, Entity, FontWeight, IntoElement, Overflow, PointRefinement,
    SharedString, Stateful, div, prelude::*, px,
};
use gpui_tesserae::{
    ElementIdExt,
    components::ChatBubble,
    primitives::{MinW0Wrapper, min_w0_wrapper},
    theme::ThemeExt,
};

use crate::managers::Chat;

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
        .items_end()
        .child(child)
}

fn render_messages<'a>(chat: &'a Chat, cx: &'a App) -> impl Iterator<Item = AnyElement> + 'a {
    chat.read_messages(cx)
        .iter()
        .map(|(_id, message)| render_message(&message.message, cx))
}

fn render_message(message: &Message, cx: &App) -> AnyElement {
    let content = SharedString::from(&message.content);

    match message.role {
        MessageRole::User => {
            right_align(ChatBubble::new("chat_bubble").child(content)).into_any_element()
        }
        _ => text_wrapper(cx).child(content).into_any_element(),
    }
}

fn text_wrapper(cx: &App) -> MinW0Wrapper {
    let text_size = cx.get_theme().layout.text.default_font.sizes.heading_sm;
    let primary_text_color = cx.get_theme().variants.active(cx).colors.text.primary;

    min_w0_wrapper()
        .font_family("Geist")
        .text_color(primary_text_color)
        .text_size(text_size)
        .font_weight(FontWeight::NORMAL)
}
