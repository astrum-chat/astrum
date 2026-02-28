use anyml::{ChatChunk, ChatOptions, MessageRole, models::Message};
use futures::future::{AbortHandle, Abortable};
use gpui::{App, AsyncApp, Entity, SharedString};
use gpui_tesserae::primitives::input::InputState;
use notitia::prelude::*;
use serde_json::value::RawValue;

use schema::{AstrumDb, MessageRecord, UniqueId};

use crate::managers::{Managers, ChatsManager};
use crate::utils::errors::push_error_async;

pub(super) fn handle_submit(
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

    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let chat_id_for_handle = chat_id.clone();
    managers.chats.update(cx, |chats, _cx| {
        chats.set_title_abort_handle(chat_id_for_handle, abort_handle);
    });

    let user_message = contents.to_string();
    let chats = managers.chats.clone();
    let errors = managers.errors.clone();
    let chat_id = chat_id.clone();

    cx.spawn(async move |cx: &mut AsyncApp| {
        let title_future = async {
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
        };

        let _ = Abortable::new(title_future, abort_registration).await;

        let _ = chats.update(cx, |chats: &mut ChatsManager, _cx| {
            chats.clear_title_abort_handle(&chat_id);
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
