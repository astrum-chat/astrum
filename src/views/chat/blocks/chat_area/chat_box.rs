use anyml::models::{Model, ModelParams, ModelQuant};
use gpui::{
    App, ElementId, IntoElement, RenderOnce, SharedString, Window, deferred, div, prelude::*, px,
    radians, relative,
};

use gpui_tesserae::{
    ElementIdExt, PositionalParentElement, TesseraeIconKind,
    components::{Button, Icon, Input, Toggle, ToggleVariant, select::SelectMenu},
    extensions::mouse_handleable::MouseHandleable,
    primitives::input::InputState,
    theme::{ThemeExt, ThemeLayerKind},
};

use crate::{assets::AstrumIconKind, blocks::ModelPicker, managers::Managers};

use super::chat_actions::send_message;

#[derive(IntoElement)]
pub struct ChatBox {
    id: ElementId,
    managers: Managers,
}

impl ChatBox {
    pub fn new(id: impl Into<ElementId>, managers: Managers) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl RenderOnce for ChatBox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let primary_text_color = cx.get_theme().variants.active(cx).colors.text.primary;
        let text_heading_sm_size = cx.get_theme().layout.text.default_font.sizes.heading_sm;

        let chat_box_input_state = window.use_state(cx, |_window, cx| InputState::new(cx));

        let models_cache = self.managers.models.read(cx).models_cache.clone();

        let model_picker = ModelPicker::new(
            self.id.clone(),
            self.managers.clone(),
            models_cache.clone(),
            None,
            window,
            cx,
        );

        let models_state_for_toggle = model_picker.state.clone();
        let models_state_for_menu = model_picker.state.clone();

        let menu_visible_delta = model_picker
            .state
            .menu_visible_transition
            .evaluate(window, cx)
            .value();

        let current_provider_icon: Option<SharedString> =
            self.managers.models.read_with(cx, |models, cx| {
                models
                    .get_current_provider(cx)
                    .map(|p| p.icon.read(cx).clone())
            });

        let chat_box_left_items = div()
            .max_w_full()
            .child(deferred(
                Toggle::new(self.id.with_suffix("switch_llm_btn"))
                    .w_auto()
                    .max_w(relative(1.))
                    .variant(ToggleVariant::Secondary)
                    .checked(
                        model_picker.state.menu_visible_transition.read_goal(cx) == &true.into(),
                    )
                    .disabled(model_picker.has_no_providers)
                    .when_some(current_provider_icon, |this, icon_path| {
                        this.child_left(
                            Icon::new(icon_path).size(px(14.)).color(primary_text_color),
                        )
                    })
                    .text(
                        models_state_for_toggle
                            .get_selected_item_name(cx)
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| {
                                self.managers.models.read_with(cx, |models, cx| {
                                    if models.providers.read(cx).is_empty() {
                                        return "No provider exists".to_string();
                                    }
                                    match models.current_model.read(cx).as_ref() {
                                        Some(p) => {
                                            let parameters = p
                                                .parameters
                                                .as_deref()
                                                .filter(|s| !s.is_empty())
                                                .map(|s| ModelParams::new(s));
                                            let quantization = p
                                                .quantization
                                                .as_deref()
                                                .filter(|s| !s.is_empty())
                                                .map(|s| ModelQuant::new(s));
                                            Model {
                                                id: p.model.clone(),
                                                parameters,
                                                quantization,
                                                thinking: None,
                                            }
                                            .to_string()
                                        }
                                        None => "No model selected".to_string(),
                                    }
                                })
                            }),
                    )
                    .child_right(
                        Icon::new(TesseraeIconKind::ArrowDown)
                            .color(primary_text_color)
                            .size(px(11.))
                            .map(|this| {
                                let rotation = radians(
                                    ((1. - menu_visible_delta) * 180.) * std::f32::consts::PI
                                        / 180.0,
                                );
                                this.rotate(rotation)
                            }),
                    )
                    .on_click(move |_checked, _window, cx| {
                        models_state_for_toggle.toggle_menu(cx);
                    }),
            ))
            .child(
                div()
                    .w(px(250.))
                    .absolute()
                    .bottom_full()
                    .left_0()
                    .pb(cx.get_theme().layout.padding.md)
                    .child(
                        SelectMenu::new(
                            self.id.with_suffix("models_select_menu"),
                            models_state_for_menu,
                        )
                        .layer(ThemeLayerKind::Quaternary)
                        .w(px(250.))
                        .max_h(px(350.)),
                    ),
            );

        let is_streaming = self
            .managers
            .chats
            .read_with(cx, |chats, cx| *chats.is_streaming.read(cx));
        let thinking_enabled = self
            .managers
            .chats
            .read_with(cx, |chats, cx| *chats.thinking_enabled.read(cx));
        let has_input_text = !chat_box_input_state.read(cx).value().is_empty();

        let model_supports_thinking = {
            let pair = self.managers.models.read(cx).current_model.read(cx).clone();
            match pair {
                Some(p) => models_cache
                    .read(cx)
                    .model_supports_thinking(&p.provider_id, &p.model),
                None => false,
            }
        };

        let submit_disabled = model_picker.has_no_providers
            || model_picker.has_no_model
            || (!is_streaming && !has_input_text);

        let chat_box_right_items = div()
            .flex()
            .flex_row_reverse()
            .flex_wrap()
            .flex_grow()
            .gap(px(7.))
            .child(
                Button::new(self.id.with_suffix("send_msg_btn"))
                    .icon(if is_streaming {
                        AstrumIconKind::Stop
                    } else {
                        AstrumIconKind::Send
                    })
                    .icon_size(px(18.))
                    .p(px(9.))
                    .disabled(submit_disabled)
                    .map(|this| {
                        let chat_box_input_state = chat_box_input_state.clone();
                        let managers = self.managers.clone();

                        this.on_click(move |_event, _window, cx| {
                            send_message(&managers, &chat_box_input_state, cx);
                        })
                    }),
            )
            .child(thinking_button(
                &self.id,
                &self.managers,
                thinking_enabled,
                model_supports_thinking,
            ));

        Input::new(
            self.id.with_suffix("chat_box"),
            chat_box_input_state.clone(),
        )
        .multiline()
        .multiline_max_lines(12)
        .multiline_wrapped()
        .submit_disabled(submit_disabled)
        .on_submit({
            let chat_box_input_state = chat_box_input_state.clone();
            let managers = self.managers.clone();

            move |_window, cx| {
                send_message(&managers, &chat_box_input_state, cx);
            }
        })
        .placeholder("Type your message here...")
        .rounded(cx.get_theme().layout.corner_radii.lg)
        .gap(px(2.))
        .p(px(12.))
        .inner_pl(px(11.))
        .inner_pr(px(11.))
        .inner_pt(px(5.))
        .inner_pb(px(5.))
        .text_size(text_heading_sm_size)
        .child_bottom(
            div()
                .w_full()
                .flex()
                .flex_wrap()
                .justify_between()
                .gap(px(7.))
                .child(chat_box_left_items)
                .child(chat_box_right_items),
        )
    }
}

fn thinking_button(
    id: &ElementId,
    managers: &Managers,
    thinking_enabled: bool,
    model_supports_thinking: bool,
) -> Toggle {
    let managers = managers.clone();

    Toggle::new(id.with_suffix("thinking_btn"))
        .icon(AstrumIconKind::Think)
        .icon_size(px(18.))
        .p(px(9.))
        .variant(ToggleVariant::Secondary)
        .checked(thinking_enabled)
        .disabled(!model_supports_thinking)
        .on_click(move |_event, _window, cx| {
            let thinking = managers.chats.read(cx).thinking_enabled.clone();
            thinking.update(cx, |enabled, cx| {
                *enabled = !*enabled;
                cx.notify();
            });
        })
}
