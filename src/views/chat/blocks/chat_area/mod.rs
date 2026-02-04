use std::sync::Arc;

use anyml::{ChatOptions, MessageRole, models::Message};
use futures::future::{AbortHandle, Abortable};
use gpui::{
    App, AppContext, AsyncApp, ElementId, InteractiveElement, IntoElement, RenderOnce,
    SharedString, Window, deferred, div, prelude::*, px, radians, relative,
};
use gpui_squircle::{SquircleStyled, squircle};
use gpui_tesserae::{
    ElementIdExt, PositionalParentElement, TesseraeIconKind,
    components::{Button, Icon, Input, Toggle, ToggleVariant, select::SelectMenu},
    extensions::mouse_handleable::MouseHandleable,
    primitives::input::InputState,
    theme::{ThemeExt, ThemeLayerKind},
};
use serde_json::value::RawValue;
use smol::lock::RwLock;

use crate::{Managers, assets::AstrumIconKind, blocks::ModelPicker, managers::ValuesOnly};

mod existing_chat;
use existing_chat::render_existing_chat;

mod prompt_new_chat;
use prompt_new_chat::render_prompt_new_chat;

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
            .flex_1()
            .min_w_0()
            .h_full()
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

    // Get the models cache from the manager
    let models_cache = elem.managers.read_blocking().models.models_cache.clone();

    // Create model picker with shared setup logic (None uses default callback)
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

    // Get menu visibility for arrow rotation
    let menu_visible_delta = picker
        .state
        .menu_visible_transition
        .evaluate(window, cx)
        .value();

    let chat_box_left_items = div()
        .max_w_full()
        .child(deferred(
            Toggle::new(elem.id.with_suffix("switch_llm_btn"))
                .w_auto()
                .max_w(relative(1.))
                .variant(ToggleVariant::Secondary)
                .disabled(picker.has_no_providers)
                .text(
                    models_state_for_toggle
                        .get_selected_item_name(cx)
                        .map(|name| name.to_string())
                        .unwrap_or_else(|| {
                            let managers = elem.managers.read_blocking();
                            if managers.models.providers.read(cx).is_empty() {
                                return "No provider exists".to_string();
                            }
                            let provider_name =
                                managers.models.current_model.provider_name.read(cx).clone();
                            let model = managers.models.get_current_model(cx).cloned();
                            match (provider_name, model) {
                                (Some(pn), Some(m)) => {
                                    format!("{}/{}", pn.to_lowercase(), m)
                                }
                                (None, Some(m)) => m,
                                _ => "No model selected".to_string(),
                            }
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

    // Check if currently streaming to determine button behavior
    let is_streaming = *elem.managers.read_blocking().chats.is_streaming.read(cx);
    let has_input_text = !chat_box_input_state.read(cx).value().is_empty();

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
                .disabled(
                    picker.has_no_providers
                        || picker.has_no_model
                        || (!is_streaming && !has_input_text),
                )
                .map(|this| {
                    let chat_box_input_state = chat_box_input_state.clone();
                    let managers = elem.managers.clone();

                    this.on_click(move |_event, _window, cx| {
                        let managers_guard = managers.read_blocking();
                        let is_streaming = *managers_guard.chats.is_streaming.read(cx);

                        if is_streaming {
                            // Cancel the current streaming response
                            managers_guard.chats.cancel_streaming(cx);
                        } else {
                            // Send a new message
                            let contents =
                                chat_box_input_state.update(cx, |this, _cx| this.clear());
                            let Some(contents) = contents else { return };
                            drop(managers_guard);
                            send_message(managers.clone(), contents, cx);
                        }
                    })
                }),
        );

    Input::new(
        elem.id.with_suffix("chat_box"),
        chat_box_input_state.clone(),
    )
    .line_clamp(12)
    .word_wrap(true)
    .newline_on_shift_enter(true)
    .on_enter({
        let chat_box_input_state = chat_box_input_state.clone();
        let managers = elem.managers.clone();
        move |window, cx| {
            let managers_guard = managers.read_blocking();
            let is_streaming = *managers_guard.chats.is_streaming.read(cx);

            if is_streaming {
                // Cancel the current streaming response
                managers_guard.chats.cancel_streaming(cx);
            }

            // Don't send if no provider or model is selected
            if managers_guard.models.get_current_provider(cx).is_none()
                || managers_guard.models.get_current_model(cx).is_none()
            {
                return;
            }

            let contents = chat_box_input_state.update(cx, |this, _cx| this.clear());

            window.blur();

            let Some(contents) = contents else { return };
            drop(managers_guard);
            send_message(managers.clone(), contents, cx);
        }
    })
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
            .max_w_full()
            .flex()
            .min_h_auto()
            .justify_between()
            .flex_wrap()
            .gap(px(7.))
            .child(chat_box_left_items)
            .child(chat_box_right_items),
    )
}

fn send_message(
    managers: Arc<RwLock<Managers>>,
    contents: SharedString,
    cx: &mut App,
) -> Option<()> {
    let managers_guard = managers.read_blocking();
    let current_provider = managers_guard.models.get_current_provider(cx).cloned()?;
    let current_model = managers_guard.models.get_current_model(cx).cloned()?;

    let (current_chat, is_new_chat) = match managers_guard.chats.get_current_chat(cx) {
        Ok(Some(current_chat)) => (current_chat, false),
        Ok(None) => match managers_guard.chats.create_chat(cx) {
            Ok(new_chat) => (new_chat, true),
            _ => return None,
        },
        Err(_) => return None,
    };

    managers_guard
        .chats
        .set_current_chat(cx, current_chat.read(cx).chat_id.clone());

    // Generate title for new chats if chat_titles_model is configured
    if is_new_chat {
        let chat_titles_provider = managers_guard.models.get_chat_titles_provider(cx).cloned();
        let chat_titles_model = managers_guard.models.get_chat_titles_model(cx).cloned();

        if let (Some(provider), Some(model)) = (chat_titles_provider, chat_titles_model) {
            let user_message = contents.to_string();
            let chat_for_title = current_chat.clone();

            cx.spawn(async move |cx: &mut AsyncApp| {
                let prompt = format!(
                    "Summarize this into a short 4-6 word thread title. Do not use any punctuation. Keep it natural and concise.\n\nUser: \"{}\"\nTitle:",
                    user_message
                );

                let messages = [Message {
                    content: prompt,
                    role: MessageRole::User,
                }];
                let options = ChatOptions::new(&model).messages(&messages);

                if let Ok(mut response) = provider.inner.chat(&options).await {
                    let mut title = String::new();
                    while let Some(Ok(chunk)) = response.next().await {
                        title.push_str(&chunk.content);

                        // Stream the title update to the UI
                        let current_title = title.trim().to_string();
                        if !current_title.is_empty() {
                            let _ = chat_for_title.update(cx, |chat, cx| {
                                chat.title.update(cx, |t, cx| {
                                    *t = current_title;
                                    cx.notify();
                                });
                            });
                        }
                    }

                    // Final update to persist to database
                    let final_title = title.trim().to_string();
                    if !final_title.is_empty() {
                        let _ = chat_for_title.update(cx, |chat, cx| {
                            let _ = chat.set_title(cx, final_title);
                        });
                    }
                }
            })
            .detach();
        }
    }

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

    // Set streaming state to true and create abort handle
    managers_guard.chats.set_streaming(cx, true);
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    managers_guard
        .chats
        .set_abort_handle(cx, Some(abort_handle));

    // Drop the read guard before spawning the async task
    drop(managers_guard);

    let managers_for_cleanup = managers.clone();

    cx.spawn(async move |cx: &mut AsyncApp| {
        let streaming_future = async {
            let Ok(messages) = cx.read_entity(&current_chat, move |current_chat, cx| {
                serde_json::to_string(&ValuesOnly(&current_chat.read_messages(cx)))
            }) else {
                return;
            };

            let messages = unsafe {
                std::mem::transmute::<Box<str>, Box<RawValue>>(messages.into_boxed_str())
            };

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
        };

        // Wrap the streaming future with abort registration
        let _ = Abortable::new(streaming_future, abort_registration).await;

        // Clean up streaming state when done (whether completed or aborted)
        let _ = cx.update(|cx| {
            let managers_guard = managers_for_cleanup.read_blocking();
            managers_guard.chats.set_streaming(cx, false);
            managers_guard.chats.set_abort_handle(cx, None);
        });
    })
    .detach();

    Some(())
}
