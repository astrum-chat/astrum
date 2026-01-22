use std::{sync::Arc, time::Duration};

use gpui::{
    App, Div, ElementId, Fill, FontWeight, div, ease_out_quint, img, prelude::*, px, radians,
    relative,
};
use gpui_squircle::{SquircleStyled, squircle};
use gpui_tesserae::{
    ElementIdExt, PositionalParentElement, TesseraeIconKind,
    components::{Button, ButtonVariant, Icon, Input},
    extensions::clickable::Clickable,
    primitives::{input::InputState, min_w0_wrapper},
    theme::{ThemeExt, ThemeLayerKind},
};
use gpui_transitions::WindowUseTransition;

use crate::{
    assets::AstrumIconKind, managers::Provider,
    views::settings::blocks::settings_area::pages::providers_page::QueryBounds,
};

#[derive(IntoElement)]
pub struct ProviderSettings {
    id: ElementId,
    provider: Arc<Provider>,
}

impl ProviderSettings {
    pub fn new(id: impl Into<ElementId>, provider: Arc<Provider>) -> Self {
        Self {
            id: id.into(),
            provider,
        }
    }
}

impl RenderOnce for ProviderSettings {
    fn render(self, window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let layer_kind = ThemeLayerKind::Tertiary;
        let background_color = layer_kind.resolve(cx);
        let border_color = layer_kind.next().resolve(cx);
        let primary_text_color = cx.get_theme().variants.active(cx).colors.text.primary;
        let secondary_text_color = cx.get_theme().variants.active(cx).colors.text.secondary;
        let text_heading_sm_size = cx.get_theme().layout.text.default_font.sizes.heading_sm;
        let text_caption_size = cx.get_theme().layout.text.default_font.sizes.caption;
        let icon_size = cx.get_theme().layout.size.lg;
        let corner_radius = cx.get_theme().layout.corner_radii.lg;
        let padding = cx.get_theme().layout.padding.xl;

        let url_input_state =
            window.use_keyed_state(self.id.with_suffix("state:url_input"), cx, |_window, cx| {
                InputState::new(cx).initial_value(self.provider.url.read(cx))
            });

        let api_key_input_state = window.use_keyed_state(
            self.id.with_suffix("state:api_key_input"),
            cx,
            |_window, cx| InputState::new(cx),
        );

        let bottom_section_content_height = window.use_keyed_state(
            self.id.with_suffix("state:settings_height"),
            cx,
            |_window, _cx| px(0.),
        );

        let bottom_section_expanded_transition = window
            .use_keyed_transition(
                self.id.with_suffix("state:expanded_delta"),
                cx,
                Duration::from_millis(350),
                |_window, _cx| 0.,
            )
            .with_easing(ease_out_quint());

        let bottom_section_expanded_delta =
            *bottom_section_expanded_transition.evaluate(window, cx);

        let info = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(padding / 2.)
            .child(
                min_w0_wrapper()
                    .text_size(text_heading_sm_size)
                    .text_color(primary_text_color)
                    .line_height(relative(1.))
                    .child(self.provider.name.read(cx).clone()),
            )
            .child(
                min_w0_wrapper()
                    .text_size(text_caption_size)
                    .text_color(secondary_text_color)
                    .font_weight(FontWeight::MEDIUM)
                    .line_height(relative(1.))
                    .child(url_input_state.read(cx).value()),
            );

        let top_left_content = div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(padding)
            .child(
                img(self.provider.icon.read(cx).clone())
                    .min_w(icon_size)
                    .min_h(icon_size)
                    .size(icon_size),
            )
            .child(info);

        let top_right_content = Button::new(self.id.with_suffix("reveal_settings_btn"))
            .variant(ButtonVariant::SecondaryGhost)
            .icon(TesseraeIconKind::ArrowDown)
            .p(px(8.))
            .rounded(px(6.))
            .map(|this| {
                let bottom_section_expanded_transition = bottom_section_expanded_transition.clone();

                let this = this.on_click(move |_event, _window, cx| {
                    let new_expanded_delta = 1. - bottom_section_expanded_transition.read_goal(cx);

                    bottom_section_expanded_transition.update(cx, |this, cx| {
                        *this = new_expanded_delta;
                        cx.notify();
                    });
                });

                let rotation = radians(
                    ((1. - bottom_section_expanded_delta) * 180.) * std::f32::consts::PI / 180.0,
                );
                this.icon_rotate(rotation)
            });

        let top_content = div()
            .flex()
            .justify_between()
            .items_center()
            .p(padding)
            .gap(padding)
            .child(top_left_content)
            .child(top_right_content);

        let bottom_section = div()
            .map(|this| {
                let bottom_section_height =
                    *bottom_section_content_height.read(cx) * bottom_section_expanded_delta;

                let this = this.min_h(bottom_section_height).h(bottom_section_height);

                if bottom_section_height == px(0.) {
                    this.opacity(0.)
                } else {
                    this
                }
            })
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .h_auto()
                    .map(|this| {
                        let bottom_section_content_height = bottom_section_content_height.clone();

                        this.query_bounds(move |bounds, _window, cx| {
                            bottom_section_content_height
                                .update(cx, |height, _cx| *height = bounds.size.height)
                        })
                    })
                    .child(divider(border_color).map(|this| {
                        let height =
                            *bottom_section_content_height.read(cx) * bottom_section_expanded_delta;
                        this.opacity(if height >= px(10.) { 1. } else { 0. })
                    }))
                    .when(bottom_section_expanded_delta != 0., |this| {
                        this.child(
                            div()
                                .w_full()
                                .h_auto()
                                .flex()
                                .flex_col()
                                .gap(padding)
                                .p(padding)
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap((padding / 1.5).floor())
                                        .child(
                                            div()
                                                .text_size(text_caption_size)
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(primary_text_color)
                                                .line_height(relative(1.))
                                                .child("URL"),
                                        )
                                        .child(
                                            Input::new(
                                                self.id.with_suffix("url_input"),
                                                url_input_state,
                                            )
                                            .layer(ThemeLayerKind::Quaternary)
                                            .child_left(Icon::new(AstrumIconKind::Web))
                                            .placeholder("https://example.com"),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(padding / 1.5)
                                        .child(
                                            div()
                                                .text_size(text_caption_size)
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(primary_text_color)
                                                .line_height(relative(1.))
                                                .child("API Key"),
                                        )
                                        .child(
                                            Input::new(
                                                self.id.with_suffix("api_key_input"),
                                                api_key_input_state,
                                            )
                                            .layer(ThemeLayerKind::Quaternary)
                                            .child_left(Icon::new(AstrumIconKind::Key))
                                            .placeholder("*************************"),
                                        ),
                                ),
                        )
                    }),
            );

        div()
            .w_full()
            .h_auto()
            .flex()
            .flex_col()
            .child(
                squircle()
                    .absolute_expand()
                    .bg(background_color)
                    .border(px(1.))
                    .border_color(border_color)
                    .border_inside()
                    .rounded(corner_radius),
            )
            .child(top_content)
            .child(bottom_section)
    }
}

fn divider(color: impl Into<Fill>) -> Div {
    div().w(relative(1.)).h(px(1.)).min_h(px(1.)).bg(color)
}
