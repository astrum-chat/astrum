use std::sync::Arc;

use anyml::{ChatOptions, MessageRole};
use gpui::{
    App, AppContext, AsyncApp, ElementId, InteractiveElement, IntoElement, RenderOnce,
    SharedString, Window, deferred, div, prelude::*, px, radians,
};
use gpui_squircle::{SquircleStyled, squircle};
use gpui_tesserae::{
    ElementIdExt, PositionalParentElement, TesseraeIconKind,
    components::{Button, Icon, Input, Toggle, ToggleVariant, select::SelectMenu},
    extensions::clickable::Clickable,
    primitives::input::InputState,
    theme::{ThemeExt, ThemeLayerKind},
};
use serde_json::value::RawValue;
use smol::lock::{RwLock, RwLockReadGuard};

use crate::{Managers, assets::AstrumIconKind, managers::ValuesOnly};

mod existing_chat;
use existing_chat::render_existing_chat;

mod prompt_new_chat;
use prompt_new_chat::render_prompt_new_chat;

mod models_menu;
use models_menu::{create_models_select_state, fetch_all_models, observe_providers_for_refresh};

#[derive(IntoElement)]
pub struct ChatArea {
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
}

impl ChatArea {
    pub fn new(id: impl Into<ElementId>, managers: Arc<RwLock<Managers>>) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl RenderOnce for ChatArea {
    fn render(self, window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let secondary_bg_color = cx
            .get_theme()
            .variants
            .active(cx)
            .colors
            .background
            .secondary;

        div()
            .id(self.id.clone())
            .tab_group()
            .tab_index(1)
            .tab_stop(false)
            .size_full()
            .flex()
            .justify_center()
            .child(
                squircle()
                    .absolute_expand()
                    .rounded_tl(px(8.))
                    .rounded_tr(px(8.))
                    .bg(secondary_bg_color),
            )
            .child(
                div()
                    .h_full()
                    .w_full()
                    .max_w(px(800.))
                    .flex()
                    .flex_col()
                    .items_start()
                    .justify_between()
                    .map(|this| {
                        let managers = self.managers.read_blocking();
                        let current_chat = managers.chats.get_current_chat(cx);

                        match current_chat {
                            Ok(Some(current_chat)) => {
                                this.child(render_existing_chat(&self.id, &current_chat, cx))
                            }
                            _ => this.child(render_prompt_new_chat(window, cx)),
                        }
                    })
                    .child(
                        div()
                            .w_full()
                            .p(px(20.))
                            .pt(px(0.))
                            .child(chat_box(&self, window, cx)),
                    ),
            )
    }
}

fn chat_box(elem: &ChatArea, window: &mut Window, cx: &mut App) -> Input {
    let primary_text_color = cx.get_theme().variants.active(cx).colors.text.primary;
    let text_heading_sm_size = cx.get_theme().layout.text.default_font.sizes.heading_sm;

    let chat_box_input_state = window.use_state(cx, |_window, cx| InputState::new(cx));

    // Create models select state
    let models_select_state =
        create_models_select_state(elem.id.clone(), elem.managers.clone(), window, cx);

    // Clone for use in the toggle click handler
    let models_state_for_toggle = Arc::new(models_select_state);
    let models_state_for_menu = models_state_for_toggle.clone();
    let managers_for_toggle = elem.managers.clone();

    // Observe providers and clear menu when they change
    let providers_entity = elem.managers.read_blocking().models.providers.clone();
    observe_providers_for_refresh(
        &providers_entity,
        models_state_for_toggle.clone(),
        elem.managers.clone(),
        cx,
    );

    // Check if there are any providers (disable buttons if none)
    let has_no_providers = providers_entity.read(cx).is_empty();

    // Check if a model is selected (disable send button if not)
    let has_no_model = elem
        .managers
        .read_blocking()
        .models
        .get_current_model(cx)
        .is_none();

    // Get menu visibility for arrow rotation
    let menu_visible_delta = models_state_for_toggle
        .menu_visible_transition
        .evaluate(window, cx)
        .value();

    let chat_box_left_items = div()
        .child(deferred(
            Toggle::new(elem.id.with_suffix("switch_llm_btn"))
                .w_auto()
                .variant(ToggleVariant::Secondary)
                .disabled(has_no_providers)
                .text(
                    models_state_for_toggle
                        .get_selected_item_name(cx)
                        .map(|name| name.to_string())
                        .unwrap_or_else(|| {
                            elem.managers
                                .read_blocking()
                                .models
                                .get_current_model(cx)
                                .map(|this| this.to_string())
                                .unwrap_or_else(|| "No model selected".to_string())
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

                    // Fetch models if not already loaded
                    if models_state_for_toggle.items.read(cx).is_empty() {
                        fetch_all_models(
                            managers_for_toggle.clone(),
                            models_state_for_toggle.clone(),
                            cx,
                        );
                    }
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

    let chat_box_right_items = div().flex().flex_row_reverse().gap(px(7.)).child(
        Button::new(elem.id.with_suffix("send_msg_btn"))
            .icon(AstrumIconKind::Send)
            .icon_size(px(18.))
            .p(px(9.))
            .disabled(has_no_providers || has_no_model)
            .map(|this| {
                let chat_box_input_state = chat_box_input_state.clone();
                let managers = elem.managers.clone();

                this.on_click(move |_event, _window, cx| {
                    let contents = chat_box_input_state.update(cx, |this, _cx| this.clear());
                    let Some(contents) = contents else { return };

                    send_message(managers.read_blocking(), contents, cx);
                })
            }),
    );

    Input::new(elem.id.with_suffix("chat_box"), chat_box_input_state)
        .line_clamp(12)
        .word_wrap(true)
        .newline_on_shift_enter(true)
        .placeholder("Type your message here...")
        .rounded(cx.get_theme().layout.corner_radii.lg)
        .gap(px(4.))
        .p(px(14.))
        .inner_pl(px(11.))
        .inner_pr(px(11.))
        .inner_pt(px(5.))
        .inner_pb(px(5.))
        .text_size(text_heading_sm_size)
        .child_bottom(
            div()
                .flex()
                .items_start()
                .min_h_auto()
                .gap(px(7.))
                .justify_between()
                .flex_wrap()
                .child(chat_box_left_items)
                .child(chat_box_right_items),
        )
}

fn send_message(
    managers: RwLockReadGuard<'_, Managers>,
    contents: SharedString,
    cx: &mut App,
) -> Option<()> {
    let current_provider = managers.models.get_current_provider(cx).cloned()?;
    let current_model = managers.models.get_current_model(cx).cloned()?;

    let current_chat = match managers.chats.get_current_chat(cx) {
        Ok(Some(current_chat)) => current_chat,
        Ok(None) => match managers.chats.create_chat(cx) {
            Ok(new_chat) => new_chat,
            _ => return None,
        },
        Err(_) => return None,
    };

    managers
        .chats
        .set_current_chat(cx, current_chat.read(cx).chat_id.clone());

    let msg_id = current_chat
        .update(cx, |current_chat, cx| {
            current_chat
                .push_message(
                    cx,
                    &current_chat.chat_id.clone(),
                    contents,
                    MessageRole::User,
                )
                .unwrap();
            current_chat.push_message(
                cx,
                &current_chat.chat_id.clone(),
                "",
                MessageRole::Assistant,
            )
        })
        .ok()?;

    cx.spawn(async move |cx: &mut AsyncApp| {
        let Ok(messages) = cx.read_entity(&current_chat, move |current_chat, cx| {
            serde_json::to_string(&ValuesOnly(&current_chat.read_messages(cx)))
        }) else {
            return;
        };

        let messages =
            unsafe { std::mem::transmute::<Box<str>, Box<RawValue>>(messages.into_boxed_str()) };

        let options = ChatOptions::new(&current_model).messages_serialized(messages);
        let response = current_provider.inner.chat(&options).await;

        match response {
            Ok(mut response) => {
                while let Some(Ok(chunk)) = response.next().await {
                    let _ = current_chat.update(cx, |current_chat, cx| {
                        current_chat
                            .push_message_content(cx, &msg_id, &chunk.content)
                            .unwrap();
                        cx.notify();
                    });
                }
            }
            Err(err) => {
                let _ = current_chat.update(cx, |current_chat, cx| {
                    current_chat
                        .push_message_content(cx, &msg_id, &format!("{}", err.to_string()))
                        .unwrap();
                    cx.notify();
                });
            }
        };
    })
    .detach();

    Some(())
}
