use std::time::Duration;

use gpui::{App, Window, div, ease_out_quint, prelude::*, px, radians};
use gpui_tesserae::{components::Icon, primitives::min_w0_wrapper, theme::ThemeExt};
use gpui_transitions::WindowUseTransition;

use crate::assets::AstrumIconKind;

pub fn render_prompt_new_chat(window: &mut Window, cx: &mut App) -> impl IntoElement {
    let secondary_text_color = cx.get_theme().variants.active(cx).colors.text.secondary;
    let text_size = cx.get_theme().layout.text.default_font.sizes.heading_sm;

    let icon_transition = window
        .use_keyed_transition(
            "prompt_new_chat:icon_transition",
            cx,
            Duration::from_millis(1000),
            |_window, _cx| 0.0_f32,
        )
        .with_easing(ease_out_quint());

    let evaluated_icon_transition = *icon_transition.evaluate(window, cx);
    let evaluated_icon_transition_radians =
        radians(evaluated_icon_transition * 2.0 * std::f32::consts::PI);

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
            div()
                .id("prompt_new_chat_icon")
                .on_click({
                    let spin_transition = icon_transition.clone();
                    move |_event, _window, cx| {
                        spin_transition.update(cx, |val, cx| {
                            *val += 1.0;
                            cx.notify();
                        });
                    }
                })
                .child(
                    Icon::new(AstrumIconKind::Logo)
                        .size(px(40.))
                        .color(secondary_text_color)
                        .rotate(evaluated_icon_transition_radians),
                ),
        )
        .child(
            min_w0_wrapper()
                .child("A Great Conversation Starts Here")
                .text_size(text_size)
                .text_center()
                .text_color(secondary_text_color),
        )
}
