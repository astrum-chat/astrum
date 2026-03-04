use gpui::{
    App, Bounds, ElementId, Hsla, IntoElement, Overflow, Pixels, PointRefinement, SharedString,
    Window, canvas, deferred, div, prelude::*, px, relative,
};
use gpui_tesserae::{
    ElementIdExt,
    components::{
        Button, Icon,
        select::{SelectItem, SelectItemsMap, SelectMenu, SelectState},
    },
    extensions::mouse_handleable::MouseHandleable,
    theme::{ThemeExt, ThemeLayerKind},
};

mod provider_settings;
use provider_settings::*;

use crate::{
    assets::AstrumIconKind,
    blocks::models_menu::{ProviderConfigChange, refetch_provider_models},
    managers::{Managers, ProviderKind},
    views::settings::blocks::settings_area::pages::render_settings_page_title,
};

#[derive(Clone)]
struct ProviderSelectItem {
    name: SharedString,
    icon_path: SharedString,
    kind: ProviderKind,
}

impl SelectItem for ProviderSelectItem {
    type Value = ProviderKind;

    fn name(&self) -> SharedString {
        self.name.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.kind
    }

    fn display(&self, _window: &mut Window, _cx: &App, text_color: Hsla) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .child(
                Icon::new(self.icon_path.clone())
                    .size(px(14.))
                    .color(text_color)
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .min_w_0()
                    .w_full()
                    .text_ellipsis()
                    .text_color(text_color)
                    .child(self.name.clone()),
            )
    }
}

#[derive(IntoElement)]
pub struct ProvidersPage {
    id: ElementId,
    managers: Managers,
}

impl ProvidersPage {
    pub fn new(id: impl Into<ElementId>, managers: Managers) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl RenderOnce for ProvidersPage {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let provider_kinds = [
            ProviderKind::Ollama,
            ProviderKind::OpenAi,
            ProviderKind::Anthropic,
            ProviderKind::ClaudeSdk,
        ];

        let mut add_provider_menu_state = SelectState::<_, ProviderSelectItem>::from_window(
            self.id.with_suffix("select_state"),
            window,
            cx,
            |_window, cx| {
                let mut map = SelectItemsMap::new();

                for kind in provider_kinds {
                    map.push_item(cx, ProviderSelectItem {
                        name: kind.default_name(),
                        icon_path: kind.default_icon(),
                        kind,
                    });
                }

                map
            },
        );

        let managers = self.managers.clone();
        add_provider_menu_state.on_item_click(move |_checked, state, item_name, _window, cx| {
            let kind = match item_name.as_ref() {
                "Ollama" => ProviderKind::Ollama,
                "OpenAI" => ProviderKind::OpenAi,
                "Anthropic" => ProviderKind::Anthropic,
                "Claude SDK" => ProviderKind::ClaudeSdk,
                _ => return,
            };

            let name = kind.default_name();
            let url = kind.default_url();

            let errors = managers.errors.clone();
            let provider_id = managers.models.update(cx, |models, cx| {
                models.new_provider(cx, kind, name, url, None, errors)
            });

            // Fetch models for the newly created provider
            refetch_provider_models(
                managers.clone(),
                provider_id,
                ProviderConfigChange::Create,
                cx,
            );

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
                let providers = self.managers.models.read(cx).providers.read(cx);

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
                "Press the '+' button in the top right corner to add an inference provider.",
            ),
        )
}
