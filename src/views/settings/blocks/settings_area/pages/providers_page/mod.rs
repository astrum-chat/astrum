use std::sync::Arc;

use gpui::{
    App, Bounds, ElementId, Overflow, Pixels, PointRefinement, Window, canvas, deferred, div,
    prelude::*, px, relative,
};
use gpui_tesserae::{
    ElementIdExt,
    components::{
        Button,
        select::{SelectItemsMap, SelectMenu, SelectState},
    },
    extensions::mouse_handleable::MouseHandleable,
    theme::{ThemeExt, ThemeLayerKind},
};
use smol::lock::RwLock;

mod provider_settings;
use provider_settings::*;

use crate::{
    assets::AstrumIconKind,
    blocks::models_menu::{ProviderConfigChange, refetch_provider_models},
    managers::{Managers, ProviderKind},
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

        let managers = self.managers.clone();
        add_provider_menu_state.on_item_click(move |_checked, state, item_name, _window, cx| {
            let kind = match item_name.as_ref() {
                "Ollama" => ProviderKind::Ollama,
                "OpenAI" => ProviderKind::OpenAi,
                "Anthropic" => ProviderKind::Anthropic,
                _ => return,
            };

            let name = kind.default_name();
            let url = kind.default_url();
            let icon = kind.default_icon();

            let provider_id = managers.write_arc_blocking().models.new_provider(
                cx,
                kind,
                name,
                url,
                Some(icon.to_string()),
                None,
            );

            if let Ok(provider_id) = provider_id {
                // Fetch models for the newly created provider
                refetch_provider_models(
                    managers.clone(),
                    provider_id,
                    ProviderConfigChange::Create,
                    cx,
                );
            }

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
                            let element_id = self
                                .id
                                .with_suffix("provider")
                                .with_suffix(provider_id.to_string());
                            ProviderSettings::new(
                                element_id,
                                self.managers.clone(),
                                provider_id.clone(),
                                provider.clone(),
                            )
                        }))
                        .into_any_element(),
                }
            })
    }
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
