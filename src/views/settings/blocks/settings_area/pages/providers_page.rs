use std::{sync::Arc, time::Duration};

use gpui::{
    App, Bounds, Div, ElementId, Fill, FontWeight, Overflow, Pixels, PointRefinement, Window,
    canvas, deferred, div, ease_out_quint, img, prelude::*, px, radians, relative,
};
use gpui_squircle::{SquircleStyled, squircle};
use gpui_tesserae::{
    ElementIdExt, PositionalParentElement, TesseraeIconKind,
    components::{
        Button, ButtonVariant, Icon, Input,
        select::{SelectItemsMap, SelectMenu, SelectState},
    },
    extensions::clickable::Clickable,
    primitives::{input::InputState, min_w0_wrapper},
    theme::{ThemeExt, ThemeLayerKind},
};
use gpui_transitions::WindowUseTransition;
use smol::lock::RwLock;

use crate::{
    assets::AstrumIconKind,
    managers::{Managers, Provider},
    views::settings::blocks::settings_area::pages::render_settings_page_title,
};

#[derive(IntoElement)]
pub struct ProvidersPage {
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
}

impl ProvidersPage {
    pub fn new(id: impl Into<ElementId>, managers: Arc<RwLock<Managers>>) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl RenderOnce for ProvidersPage {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let mut add_provider_menu_state = SelectState::<_, &'static str>::from_window(
            self.id.with_suffix("select_state"),
            window,
            cx,
            |_window, cx| {
                let mut map = SelectItemsMap::new();

                map.push_item(cx, "Ollama");
                map.push_item(cx, "OpenAI");
                map.push_item(cx, "Anthropic");

                map
            },
        );

        add_provider_menu_state.on_item_click(|_checked, state, _item_name, _window, cx| {
            state.hide_menu(cx);
        });

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap(px(20.))
                    .child(render_settings_page_title(
                        cx,
                        "Providers",
                        "Manage and configure inference providers.",
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .child(
                                deferred(
                                    Button::new(self.id.with_suffix("add_provider_btn"))
                                        .icon(AstrumIconKind::ThickPlus)
                                        .icon_size(px(14.))
                                        .p(px(8.))
                                        .rounded(px(6.))
                                        // This event handler solely exists to ensure event propagation is stoped.
                                        .on_any_mouse_down(|_event, _window, _cx| ())
                                        .map(|this| {
                                            let menu_visible_transition = add_provider_menu_state
                                                .menu_visible_transition
                                                .clone();

                                            this.on_click(move |_event, _window, cx| {
                                                menu_visible_transition.update(cx, |this, cx| {
                                                    *this = this.toggle();
                                                    cx.notify();
                                                });
                                            })
                                        }),
                                )
                                .priority(1),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .w(px(175.))
                                    .absolute()
                                    .top_full()
                                    .right_0()
                                    .pt(cx.get_theme().layout.padding.md)
                                    .child(
                                        SelectMenu::new(
                                            self.id.with_suffix("add_provider_menu"),
                                            add_provider_menu_state,
                                        )
                                        .layer(ThemeLayerKind::Quaternary),
                                    ),
                            ),
                    ),
            )
            .child({
                let providers = self.managers.read_arc_blocking().models.providers.read(cx);

                match providers.len() {
                    0 => render_prompt_create_first_provider(cx).into_any_element(),
                    _ => div()
                        .id(self.id.clone())
                        .w_full()
                        .h_full()
                        .flex()
                        .flex_col()
                        .pb(px(20.))
                        .gap(px(10.))
                        .map(|mut this| {
                            this.style().overflow = PointRefinement {
                                x: None,
                                y: Some(Overflow::Scroll),
                            };
                            this
                        })
                        .children(providers.iter().map(|(provider_id, provider)| {
                            let provider_id = self
                                .id
                                .with_suffix("provider")
                                .with_suffix(provider_id.to_string());
                            ProviderSettings::new(provider_id, provider.clone())
                        }))
                        .into_any_element(),
                }
            })
    }
}

#[derive(IntoElement)]
struct ProviderSettings {
    id: ElementId,
    provider: Arc<Provider>,
}

impl ProviderSettings {
    fn new(id: impl Into<ElementId>, provider: Arc<Provider>) -> Self {
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

trait QueryBounds {
    fn query_bounds(
        self,
        query: impl FnMut(Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self;
}

impl<E: IntoElement + ParentElement> QueryBounds for E {
    fn query_bounds(
        self,
        mut query: impl FnMut(Bounds<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.child(
            canvas(
                |_bounds, _window, _cx| {},
                move |bounds, _, window, cx| query(bounds, window, cx),
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
    }
}

fn render_prompt_create_first_provider(cx: &App) -> impl IntoElement {
    let secondary_text_color = cx.get_theme().variants.active(cx).colors.text.secondary;
    let body_size = cx.get_theme().layout.text.default_font.sizes.body;

    div()
        .text_color(secondary_text_color)
        .text_size(body_size)
        .w_full()
        .h(relative(0.75))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(
            div().w_full().min_w_0().h_auto().text_center().child(
                "Press the '+' button in the top right corner to add an inferance provider.",
            ),
        )
}
