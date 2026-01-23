use std::time::{Duration, Instant};

use gpui::{App, Window, div, ease_out_quint, prelude::*, px, radians};
use gpui_tesserae::{components::Icon, primitives::min_w0_wrapper, theme::ThemeExt};
use gpui_transitions::WindowUseTransition;

use crate::assets::AstrumIconKind;
use crate::utils::strings::choose_string;

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

    let click_count =
        window.use_keyed_state("prompt_new_chat:click_count", cx, |_window, _cx| 0u32);

    let display_state =
        window.use_keyed_state("prompt_new_chat:display_state", cx, |_window, _cx| {
            (None::<&'static str>, None::<u8>, None::<Instant>)
        });

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
                    let click_count = click_count.clone();
                    let display_state = display_state.clone();
                    move |_event, _window, cx| {
                        spin_transition.update(cx, |val, cx| {
                            *val += 1.0;
                            cx.notify();
                        });

                        let count = *click_count.read(cx);
                        click_count.update(cx, |val, _cx| *val += 1);

                        let (_, last_tier, last_change) = *display_state.read(cx);
                        if let Some((message, tier)) = choose_string(count, last_tier, last_change)
                        {
                            display_state.update(cx, |val, _cx| {
                                *val = (Some(message), Some(tier), Some(Instant::now()))
                            });
                        }
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
                .child(
                    display_state
                        .read(cx)
                        .0
                        .unwrap_or("A Great Conversation Starts Here"),
                )
                .text_size(text_size)
                .text_center()
                .text_color(secondary_text_color),
        )
}
