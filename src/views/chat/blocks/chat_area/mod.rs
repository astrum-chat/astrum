use anyml::models::{Model, ModelParams, ModelQuant};
use gpui::{
    App, ElementId, InteractiveElement, IntoElement, RenderOnce, SharedString, Window, deferred,
    div, prelude::*, px, radians, relative,
};

use gpui_tesserae::{
    ElementIdExt, PositionalParentElement, TesseraeIconKind,
    components::{Button, Icon, Input, Toggle, ToggleVariant, select::SelectMenu},
    extensions::mouse_handleable::MouseHandleable,
    primitives::input::InputState,
    theme::{ThemeExt, ThemeLayerKind},
};
use notitia::prelude::*;
use notitia_gpui::WindowNotitiaExt;

use schema::{AstrumDb, MessageRecord};

use crate::{assets::AstrumIconKind, blocks::ModelPicker, managers::Managers};

mod chat_actions;
use chat_actions::handle_submit;

mod existing_chat;
mod md_render;
use existing_chat::render_existing_chat;

mod prompt_new_chat;
use prompt_new_chat::render_prompt_new_chat;

#[derive(IntoElement)]
pub struct ChatArea {
    id: ElementId,
    managers: Managers,
}

impl ChatArea {
    pub fn new(id: impl Into<ElementId>, managers: Managers) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl RenderOnce for ChatArea {
    fn render(self, window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let (current_chat_id, db_initialized, db) =
            self.managers.chats.read_with(cx, |chats, cx| {
                (
                    chats.get_current_chat_id().read(cx).clone(),
                    chats.db_initialized(),
                    if chats.db_initialized() {
                        Some(chats.db().clone())
                    } else {
                        None
                    },
                )
            });

        let messages = current_chat_id
            .as_ref()
            .filter(|_| db_initialized)
            .and_then(|chat_id| {
                let db = db.as_ref()?;
                let chat_id_for_query = chat_id.clone();
                Some(window.use_keyed_db_query(
                    format!("messages_{}", chat_id),
                    cx,
                    |_window, _cx| {
                        db.query(
                            AstrumDb::MESSAGES
                                .select((
                                    MessageRecord::ID,
                                    MessageRecord::ROLE,
                                    MessageRecord::CONTENT,
                                ))
                                .filter(MessageRecord::CHAT_ID.eq(chat_id_for_query.clone()))
                                .order_by(MessageRecord::CREATED_AT, OrderDirection::Asc)
                                .fetch_all::<BTreeMap<_, _>>(),
                        )
                    },
                ))
            });

        div()
            .id(self.id.clone())
            .h_full()
            .w_full()
            .max_w(px(800.))
            .flex()
            .flex_col()
            .items_start()
            .justify_between()
            .map(|this| {
                match &messages {
                    Some(messages) => match messages.read(cx) {
                        Some(msgs) => this.child(render_existing_chat(&self.id, msgs)),
                        None => this.child(div()), // spacer so justify_between keeps chat box at bottom
                    },
                    None => this.child(render_prompt_new_chat(window, cx)),
                }
            })
            .child(
                div()
                    .w_full()
                    .p(px(20.))
                    .pt(px(0.))
                    .child(chat_box(&self, window, cx)),
            )
    }
}

fn chat_box(elem: &ChatArea, window: &mut Window, cx: &mut App) -> Input {
    let primary_text_color = cx.get_theme().variants.active(cx).colors.text.primary;
    let text_heading_sm_size = cx.get_theme().layout.text.default_font.sizes.heading_sm;

    let chat_box_input_state = window.use_state(cx, |_window, cx| InputState::new(cx));

    let models_cache = elem.managers.models.read(cx).models_cache.clone();

    let picker = ModelPicker::new(
        elem.id.clone(),
        elem.managers.clone(),
        models_cache.clone(),
        None,
        window,
        cx,
    );

    let models_state_for_toggle = picker.state.clone();
    let models_state_for_menu = picker.state.clone();

    let menu_visible_delta = picker
        .state
        .menu_visible_transition
        .evaluate(window, cx)
        .value();

    let current_provider_icon: Option<SharedString> =
        elem.managers.models.read_with(cx, |models, cx| {
            models
                .get_current_provider(cx)
                .map(|p| p.icon.read(cx).clone())
        });

    let chat_box_left_items = div()
        .max_w_full()
        .child(deferred(
            Toggle::new(elem.id.with_suffix("switch_llm_btn"))
                .w_auto()
                .max_w(relative(1.))
                .variant(ToggleVariant::Secondary)
                .checked(picker.state.menu_visible_transition.read_goal(cx) == &true.into())
                .disabled(picker.has_no_providers)
                .when_some(current_provider_icon, |this, icon_path| {
                    this.child_left(Icon::new(icon_path).size(px(14.)).color(primary_text_color))
                })
                .text(
                    models_state_for_toggle
                        .get_selected_item_name(cx)
                        .map(|name| name.to_string())
                        .unwrap_or_else(|| {
                            elem.managers.models.read_with(cx, |models, cx| {
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
                                ((1. - menu_visible_delta) * 180.) * std::f32::consts::PI / 180.0,
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
                        elem.id.with_suffix("models_select_menu"),
                        models_state_for_menu,
                    )
                    .layer(ThemeLayerKind::Quaternary)
                    .w(px(250.))
                    .max_h(px(350.)),
                ),
        );

    let is_streaming = elem
        .managers
        .chats
        .read_with(cx, |chats, cx| *chats.is_streaming.read(cx));
    let thinking_enabled = elem
        .managers
        .chats
        .read_with(cx, |chats, cx| *chats.thinking_enabled.read(cx));
    let has_input_text = !chat_box_input_state.read(cx).value().is_empty();

    let model_supports_thinking = {
        let pair = elem.managers.models.read(cx).current_model.read(cx).clone();
        match pair {
            Some(p) => models_cache
                .read(cx)
                .model_supports_thinking(&p.provider_id, &p.model),
            None => false,
        }
    };

    let submit_disabled =
        picker.has_no_providers || picker.has_no_model || (!is_streaming && !has_input_text);

    let chat_box_right_items = div()
        .flex()
        .flex_row_reverse()
        .flex_wrap()
        .flex_grow()
        .gap(px(7.))
        .child(
            Button::new(elem.id.with_suffix("send_msg_btn"))
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
                    let managers = elem.managers.clone();

                    this.on_click(move |_event, _window, cx| {
                        handle_submit(&managers, &chat_box_input_state, cx);
                    })
                }),
        )
        .child({
            let managers = elem.managers.clone();
            Toggle::new(elem.id.with_suffix("thinking_btn"))
                .icon(AstrumIconKind::Think)
                .icon_size(px(18.))
                .p(px(9.))
                .variant(ToggleVariant::Constructive)
                .checked(thinking_enabled)
                .disabled(!model_supports_thinking)
                .on_click(move |_event, _window, cx| {
                    let thinking = managers.chats.read(cx).thinking_enabled.clone();
                    thinking.update(cx, |enabled, cx| {
                        *enabled = !*enabled;
                        cx.notify();
                    });
                })
        });

    Input::new(
        elem.id.with_suffix("chat_box"),
        chat_box_input_state.clone(),
    )
    .multiline()
    .multiline_max_lines(12)
    .multiline_wrapped()
    .submit_disabled(submit_disabled)
    .on_submit({
        let chat_box_input_state = chat_box_input_state.clone();
        let managers = elem.managers.clone();

        move |_window, cx| {
            handle_submit(&managers, &chat_box_input_state, cx);
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
