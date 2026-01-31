use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
    time::Duration,
};

use gpui::{
    App, Div, ElementId, Entity, Fill, Focusable, FontWeight, div, ease_out_quint, img, prelude::*,
    px, radians, relative,
};
use gpui_squircle::{SquircleStyled, squircle};
use gpui_tesserae::{
    ElementIdExt, PositionalParentElement, TesseraeIconKind,
    components::{Button, ButtonVariant, Icon, Input},
    extensions::mouse_handleable::MouseHandleable,
    primitives::{input::InputState, min_w0_wrapper},
    theme::{ThemeExt, ThemeLayerKind},
};
use gpui_transitions::WindowUseTransition;

use smol::lock::RwLock;

use crate::{
    assets::AstrumIconKind,
    blocks::models_menu::refetch_provider_models,
    managers::{Managers, Provider, UniqueId},
    views::settings::blocks::settings_area::pages::providers_page::QueryBounds,
};

/// Compute a hash for the API key to compare without storing the actual key
fn hash_api_key(api_key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    api_key.hash(&mut hasher);
    hasher.finish()
}

/// Cached state for detecting changes to provider settings
#[derive(Clone)]
struct CachedProviderState {
    url: String,
    api_key_hash: u64,
}

fn save_provider_url(
    managers: &Arc<RwLock<Managers>>,
    provider_id: &UniqueId,
    url_input_state: &Entity<InputState>,
    cached_state: &Entity<CachedProviderState>,
    cx: &mut App,
) {
    let new_url = url_input_state.read(cx).value().to_string();
    let cached_url = cached_state.read(cx).url.clone();

    // Only save and refetch if URL actually changed
    if new_url == cached_url {
        return;
    }

    {
        let mut managers_guard = managers.write_arc_blocking();
        let _ = managers_guard
            .models
            .edit_provider_url(cx, provider_id.clone(), new_url.clone());
    }

    // Update cached URL
    cached_state.update(cx, |state, _| {
        state.url = new_url;
    });

    // Refetch models for this provider since URL changed
    refetch_provider_models(managers.clone(), provider_id.clone(), cx);
}

fn save_provider_api_key(
    managers: &Arc<RwLock<Managers>>,
    provider_id: &UniqueId,
    api_key_input_state: &Entity<InputState>,
    cached_state: &Entity<CachedProviderState>,
    cx: &mut App,
) {
    let new_api_key = api_key_input_state.read(cx).value().to_string();
    let new_api_key_hash = hash_api_key(&new_api_key);
    let cached_api_key_hash = cached_state.read(cx).api_key_hash;

    // Only save and refetch if API key actually changed
    if new_api_key_hash == cached_api_key_hash {
        return;
    }

    let api_key = if new_api_key.is_empty() {
        None
    } else {
        Some(new_api_key)
    };

    {
        let mut managers_guard = managers.write_arc_blocking();
        let _ = managers_guard
            .models
            .edit_provider_api_key(cx, provider_id.clone(), api_key);
    }

    // Update cached API key hash
    cached_state.update(cx, |state, _| {
        state.api_key_hash = new_api_key_hash;
    });

    // Refetch models for this provider since API key changed
    refetch_provider_models(managers.clone(), provider_id.clone(), cx);
}

#[derive(IntoElement)]
pub struct ProviderSettings {
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
    provider_id: UniqueId,
    provider: Arc<Provider>,
}

impl ProviderSettings {
    pub fn new(
        id: impl Into<ElementId>,
        managers: Arc<RwLock<Managers>>,
        provider_id: UniqueId,
        provider: Arc<Provider>,
    ) -> Self {
        Self {
            id: id.into(),
            managers,
            provider_id,
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
            |_window, cx| {
                let api_key = self
                    .managers
                    .read_arc_blocking()
                    .models
                    .get_provider_api_key(cx, &self.provider_id)
                    .unwrap_or_default();

                InputState::new(cx).initial_value(api_key)
            },
        );

        // Cache the initial URL and API key hash to detect changes
        let cached_provider_state = window.use_keyed_state(
            self.id.with_suffix("state:cached_provider"),
            cx,
            |_window, cx| {
                let url = self.provider.url.read(cx).to_string();
                let api_key = self
                    .managers
                    .read_arc_blocking()
                    .models
                    .get_provider_api_key(cx, &self.provider_id)
                    .unwrap_or_default();

                CachedProviderState {
                    url,
                    api_key_hash: hash_api_key(&api_key),
                }
            },
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

        let expand_button = Button::new(self.id.with_suffix("reveal_settings_btn"))
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

        let delete_button = {
            let managers = self.managers.clone();
            let provider_id = self.provider_id.clone();

            Button::new(self.id.with_suffix("delete_btn"))
                .variant(ButtonVariant::DestructiveGhost)
                .icon(AstrumIconKind::Trash)
                .p(px(8.))
                .rounded(px(6.))
                .on_click(move |_event, _window, cx| {
                    let _ = managers
                        .write_arc_blocking()
                        .models
                        .delete_provider(cx, provider_id.clone());
                })
        };

        let top_right_content = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(padding / 3.)
            .child(delete_button)
            .child(expand_button);

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
                        let url_input =
                            Input::new(self.id.with_suffix("url_input"), url_input_state.clone())
                                .layer(ThemeLayerKind::Quaternary)
                                .child_left(Icon::new(AstrumIconKind::Web))
                                .placeholder("https://example.com");

                        let api_key_input = Input::new(
                            self.id.with_suffix("api_key_input"),
                            api_key_input_state.clone(),
                        )
                        .layer(ThemeLayerKind::Quaternary)
                        .child_left(Icon::new(AstrumIconKind::Key))
                        .placeholder("*************************")
                        .transform_text(|_| '*');

                        let managers = self.managers.clone();
                        let provider_id = self.provider_id.clone();

                        let _subs = window.use_keyed_state(
                            self.id.with_suffix("state:input_subs"),
                            cx,
                            |window, cx| {
                                {
                                    let managers = managers.clone();
                                    let provider_id = provider_id.clone();
                                    let url_input_state = url_input_state.clone();

                                    window
                                        .on_focus_out(
                                            &url_input.focus_handle(cx),
                                            cx,
                                            move |_event, _window, cx| {
                                                save_provider_url(
                                                    &managers,
                                                    &provider_id,
                                                    &url_input_state,
                                                    cx,
                                                );
                                            },
                                        )
                                        .detach();
                                }

                                {
                                    let managers = managers.clone();
                                    let provider_id = provider_id.clone();
                                    let api_key_input_state = api_key_input_state.clone();

                                    window
                                        .on_focus_out(
                                            &api_key_input.focus_handle(cx),
                                            cx,
                                            move |_event, _window, cx| {
                                                save_provider_api_key(
                                                    &managers,
                                                    &provider_id,
                                                    &api_key_input_state,
                                                    cx,
                                                );
                                            },
                                        )
                                        .detach();
                                }

                                {
                                    let managers = managers.clone();
                                    let provider_id = provider_id.clone();
                                    let url_input_state = url_input_state.clone();
                                    let api_key_input_state = api_key_input_state.clone();

                                    window.on_window_should_close(cx, move |_window, cx| {
                                        save_provider_url(
                                            &managers,
                                            &provider_id,
                                            &url_input_state,
                                            cx,
                                        );
                                        save_provider_api_key(
                                            &managers,
                                            &provider_id,
                                            &api_key_input_state,
                                            cx,
                                        );
                                        true
                                    });
                                }
                            },
                        );

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
                                        .child(url_input),
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
                                        .child(api_key_input),
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
