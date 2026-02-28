use anyml::{
    ChatChunk, ChatOptions, MessageRole,
    models::{Message, Model, ModelParams, ModelQuant},
};
use futures::future::{AbortHandle, Abortable};
use gpui::{
    App, AsyncApp, ElementId, Entity, InteractiveElement, IntoElement, RenderOnce, SharedString,
    Window, deferred, div, prelude::*, px, radians, relative,
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
use serde_json::value::RawValue;

use schema::{AstrumDb, MessageRecord, UniqueId};

use crate::{assets::AstrumIconKind, blocks::ModelPicker, managers::{Managers, ChatsManager}, utils::errors::push_error_async};

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
        let (current_chat_id, db_initialized, db) = self.managers.chats.read_with(cx, |chats, cx| {
            (
                chats.get_current_chat_id().read(cx).clone(),
                chats.db_initialized(),
                if chats.db_initialized() { Some(chats.db().clone()) } else { None },
            )
        });

        let messages = current_chat_id
            .as_ref()
            .filter(|_| db_initialized)
            .and_then(|chat_id| {
                let db = db.as_ref()?;
                let chat_id_for_query = chat_id.clone();
                Some(window.use_keyed_db_query(format!("messages_{}", chat_id), cx, |_window, _cx| {
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
                }))
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

    let current_provider_icon: Option<SharedString> = elem.managers.models.read_with(cx, |models, cx| {
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
                                let model_id = models.get_current_model(cx).cloned();
                                match model_id {
                                    Some(id) => {
                                        let parameters = models
                                            .current_model
                                            .parameters
                                            .read(cx)
                                            .as_ref()
                                            .filter(|p| !p.is_empty())
                                            .map(|p| ModelParams::new(p));
                                        let quantization = models
                                            .current_model
                                            .quantization
                                            .read(cx)
                                            .as_ref()
                                            .filter(|q| !q.is_empty())
                                            .map(|q| ModelQuant::new(q));
                                        Model {
                                            id,
                                            parameters,
                                            quantization,
                                            thinking: None,
                                        }
                                        .to_string()
                                    }
                                    _ => "No model selected".to_string(),
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

    let is_streaming = elem.managers.chats.read_with(cx, |chats, cx| *chats.is_streaming.read(cx));
    let has_input_text = !chat_box_input_state.read(cx).value().is_empty();

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
        );

    Input::new(
        elem.id.with_suffix("chat_box"),
        chat_box_input_state.clone(),
    )
    .multiline()
    .multiline_clamp(12)
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

fn handle_submit(
    managers: &Managers,
    chat_box_input_state: &Entity<InputState>,
    cx: &mut App,
) {
    let is_streaming = managers.chats.read_with(cx, |chats, cx| *chats.is_streaming.read(cx));

    if is_streaming {
        managers.chats.update(cx, |chats, cx| chats.cancel_streaming(cx));
        return;
    }

    let has_provider_and_model = managers.models.read_with(cx, |models, cx| {
        models.get_current_provider(cx).is_some() && models.get_current_model(cx).is_some()
    });
    if !has_provider_and_model {
        return;
    }

    let contents = chat_box_input_state.update(cx, |this: &mut InputState, _cx| this.clear());
    let Some(contents) = contents else { return };
    send_message(managers.clone(), contents, cx);
}

fn spawn_title_generation(
    managers: &Managers,
    chat_id: &UniqueId,
    contents: &SharedString,
    cx: &mut App,
) {
    let (chat_titles_provider, chat_titles_model) = managers.models.read_with(cx, |models, cx| {
        (models.get_chat_titles_provider(cx).cloned(), models.get_chat_titles_model(cx).cloned())
    });

    let (Some(provider), Some(model)) = (chat_titles_provider, chat_titles_model) else {
        return;
    };

    let user_message = contents.to_string();
    let chats = managers.chats.clone();
    let errors = managers.errors.clone();
    let chat_id = chat_id.clone();

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

        match provider.inner.chat(&options).await {
            Ok(mut response) => {
                let mut title = String::new();
                while let Some(Ok(chunk)) = response.next().await {
                    if let ChatChunk::Content(text) = chunk {
                        title.push_str(&text);
                    }

                    let current_title = title.trim().to_string();
                    if !current_title.is_empty() {
                        let _ = chats.update(cx, |chats: &mut ChatsManager, cx| {
                            chats.set_title(cx, &chat_id, &current_title);
                        });
                    }
                }
            }
            Err(e) => {
                push_error_async(&errors, cx, format!("Failed to generate chat title: {e}"));
            }
        }

        let _ = chats.update(cx, |chats: &mut ChatsManager, _cx| {
            chats.drop_mutation_queue(&chat_id);
        });
    })
    .detach();
}

fn send_message(
    managers: Managers,
    contents: SharedString,
    cx: &mut App,
) -> Option<()> {
    let (current_provider, current_model) = managers.models.read_with(cx, |models, cx| {
        (models.get_current_provider(cx).cloned(), models.get_current_model(cx).cloned())
    });
    let current_provider = current_provider?;
    let current_model = current_model?;

    let (chat_id, is_new_chat, db, assistant_msg_id, abort_registration) = managers.chats.update(cx, |chats, cx| {
        let current_chat_id = chats.get_current_chat_id().read(cx).clone();

        let (chat_id, is_new_chat) = match current_chat_id {
            Some(id) => (id, false),
            None => (UniqueId::new(), true),
        };

        chats.set_current_chat(cx, chat_id.clone());

        let assistant_msg_id = UniqueId::new();

        chats.set_streaming(cx, true);
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        chats.set_abort_handle(cx, Some(abort_handle));

        let db = chats.db().clone();

        (chat_id, is_new_chat, db, assistant_msg_id, abort_registration)
    });

    if is_new_chat {
        spawn_title_generation(&managers, &chat_id, &contents, cx);
    }

    let chat_id_for_stream = chat_id.clone();
    let chats_for_cleanup = managers.chats.clone();
    let errors = managers.errors.clone();
    let user_content = contents.to_string();

    cx.spawn(async move |cx: &mut AsyncApp| {
        let streaming_future = async {
            // Persist chat + messages to DB (sequential, no races)
            if is_new_chat {
                if let Err(e) = ChatsManager::insert_chat(&db, &chat_id_for_stream).await {
                    push_error_async(&errors, cx, format!("Failed to create chat: {e}"));
                    return;
                }
            }
            if let Err(e) = ChatsManager::insert_message(
                &db, &UniqueId::new(), &chat_id_for_stream, &user_content, MessageRole::User,
            ).await {
                push_error_async(&errors, cx, format!("Failed to save message: {e}"));
                return;
            }
            if let Err(e) = ChatsManager::insert_message(
                &db, &assistant_msg_id, &chat_id_for_stream, "", MessageRole::Assistant,
            ).await {
                push_error_async(&errors, cx, format!("Failed to save message: {e}"));
                return;
            }

            let messages_result = db
                .query(
                    AstrumDb::MESSAGES
                        .select((MessageRecord::ROLE, MessageRecord::CONTENT))
                        .filter(MessageRecord::CHAT_ID.eq(chat_id_for_stream.clone()))
                        .order_by(MessageRecord::CREATED_AT, OrderDirection::Asc)
                        .fetch_all::<BTreeMap<_, _>>(),
                )
                .execute()
                .await;

            let messages_data = match messages_result {
                Ok(data) => data,
                Err(e) => {
                    push_error_async(&errors, cx, format!("Failed to load messages: {e}"));
                    return;
                }
            };

            let api_messages: Vec<Message> = messages_data
                .values()
                .filter(|(_, content): &&(String, String)| !content.is_empty())
                .map(|(role, content)| Message {
                    role: MessageRole::from_str(role),
                    content: content.clone(),
                })
                .collect();

            let messages_json = serde_json::to_string(&api_messages).unwrap();
            let messages_raw = unsafe {
                std::mem::transmute::<Box<str>, Box<RawValue>>(messages_json.into_boxed_str())
            };

            let options = ChatOptions::new(&current_model).messages_serialized(messages_raw);
            let response = current_provider.inner.chat(&options).await;

            match response {
                Ok(mut response) => {
                    while let Some(Ok(chunk)) = response.next().await {
                        if let ChatChunk::Content(text) = chunk {
                            let _ = chats_for_cleanup.update(cx, |chats, cx| {
                                chats.push_message_content(
                                    cx,
                                    &assistant_msg_id,
                                    &text,
                                );
                            });
                        }
                    }
                }
                Err(err) => {
                    let _ = chats_for_cleanup.update(cx, |chats, cx| {
                        chats.push_message_content(
                            cx,
                            &assistant_msg_id,
                            &err.to_string(),
                        );
                    });
                }
            };
        };

        let _ = Abortable::new(streaming_future, abort_registration).await;

        let _ = chats_for_cleanup.update(cx, |chats, cx| {
            chats.drop_mutation_queue(&assistant_msg_id);
            chats.set_streaming(cx, false);
            chats.set_abort_handle(cx, None);
        });
    })
    .detach();

    Some(())
}
