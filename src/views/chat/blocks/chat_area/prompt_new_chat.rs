use gpui::{App, Window, div, prelude::*, px};
use gpui_tesserae::{components::Icon, primitives::min_w0_wrapper, theme::ThemeExt};

use crate::assets::AstrumIconKind;

pub fn render_prompt_new_chat(_window: &mut Window, cx: &mut App) -> impl IntoElement {
    let secondary_text_color = cx.get_theme().variants.active(cx).colors.text.secondary;
    let text_size = cx.get_theme().layout.text.default_font.sizes.heading_sm;

    div()
        .opacity(0.7)
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        .items_start()
        .items_center()
        .justify_center()
        .gap(px(20.))
        .child(
            Icon::new(AstrumIconKind::Logo)
                .size(px(40.))
                .color(secondary_text_color),
        )
        .child(
            min_w0_wrapper()
                .child("A Great Conversation Starts Here")
                .text_size(text_size)
                .text_center()
                .text_color(secondary_text_color),
        )
}
