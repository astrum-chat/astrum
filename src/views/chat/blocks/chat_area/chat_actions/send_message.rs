use std::sync::Arc;

use anyml::{ChatOptions, MessageRole, Thinking, models::Message};
use futures::future::{AbortHandle, Abortable};
use gpui::{App, AsyncApp, Entity};
use gpui_tesserae::primitives::input::InputState;
use notitia::Notitia;
use notitia::prelude::*;
use notitia_sqlite::SqliteAdapter;
use serde_json::value::RawValue;

use schema::{AstrumDb, MessageRecord, UniqueId};

use crate::managers::{ChatsManager, Managers, ProviderTrait};
use crate::utils::errors::push_error_async;
use crate::views::chat::blocks::chat_area::chat_actions::create_thread_title;

pub fn send_message(managers: &Managers, chat_box_input_state: &Entity<InputState>, cx: &mut App) {
    let Some((current_provider, current_model)) = managers.models.read_with(cx, |models, cx| {
        models
            .get_current_provider(cx)
            .zip(models.get_current_model(cx))
    }) else {
        return;
    };

    ensure_last_response_finished(managers, cx);

    let Some(contents) = chat_box_input_state.update(cx, |this: &mut InputState, _cx| this.clear())
    else {
        return;
    };

    let thinking_enabled = get_thinking_enabled(managers, cx);

    let (chat_id, is_new_chat, db, abort_registration) = managers.chats.update(cx, |chats, cx| {
        let (chat_id, is_new_chat) = match chats.get_current_chat_id().read(cx).as_ref() {
            Some(id) => (id.clone(), false),
            None => {
                let chat_id = UniqueId::new();

                chats.set_current_chat(cx, chat_id.clone());

                (chat_id, true)
            }
        };

        chats.set_streaming(cx, true);

        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        chats.set_abort_handle(cx, Some(abort_handle));

        let db = chats.db().clone();

        (chat_id, is_new_chat, db, abort_registration)
    });

    let assistant_msg_id = UniqueId::new();

    if is_new_chat {
        create_thread_title(managers, &chat_id, &contents, cx);
    }

    let system_prompt = managers.system_prompt.read(cx).to_string();
    let chats = managers.chats.clone();
    let errors = managers.errors.clone();
    let user_content = contents.to_string();
    let chat_id_for_stream = chat_id.clone();

    cx.spawn(async move |cx: &mut AsyncApp| {
        let streaming_future = async {
            if let Err(e) = persist_messages(
                &db,
                &chat_id_for_stream,
                &assistant_msg_id,
                &user_content,
                is_new_chat,
            )
            .await
            {
                push_error_async(&errors, cx, e);
                return;
            }

            let api_messages =
                match build_api_messages(&db, &chat_id_for_stream, &system_prompt, &errors, cx)
                    .await
                {
                    Some(msgs) => msgs,
                    None => return,
                };

            stream_chunks(
                current_provider.inner.clone(),
                &current_model,
                api_messages,
                thinking_enabled,
                &chats,
                &assistant_msg_id,
                cx,
            )
            .await;
        };

        let _ = Abortable::new(streaming_future, abort_registration).await;

        let _ = chats.update(cx, |chats, cx| {
            chats.drop_mutation_queue(&assistant_msg_id);
            chats.set_streaming(cx, false);
            chats.set_abort_handle(cx, None);
        });
    })
    .detach();
}

// Ensures the llm's last response has finished by stopping it if necessary.
fn ensure_last_response_finished(managers: &Managers, cx: &mut App) {
    let is_streaming = managers
        .chats
        .read_with(cx, |chats, cx| *chats.is_streaming.read(cx));

    if !is_streaming {
        return;
    }

    managers
        .chats
        .update(cx, |chats, cx| chats.cancel_streaming(cx));
}

fn get_thinking_enabled(managers: &Managers, cx: &mut App) -> bool {
    let thinking_is_toggled = managers
        .chats
        .read_with(cx, |chats, cx| *chats.thinking_enabled.read(cx));

    if !thinking_is_toggled {
        return false;
    };

    // Returns if the model itself supports thinking.
    managers.models.read_with(cx, |models, cx| {
        let cache = models.models_cache.read(cx);

        models
            .current_model
            .read(cx)
            .as_ref()
            .is_some_and(|p| cache.model_supports_thinking(&p.provider_id, &p.model))
    })
}

async fn persist_messages(
    db: &Notitia<AstrumDb, SqliteAdapter>,
    chat_id: &UniqueId,
    assistant_msg_id: &UniqueId,
    user_content: &str,
    is_new_chat: bool,
) -> Result<(), String> {
    if is_new_chat {
        ChatsManager::insert_chat(db, chat_id)
            .await
            .map_err(|e| format!("Failed to create chat: {e}"))?;
    }

    ChatsManager::insert_message(
        db,
        &UniqueId::new(),
        chat_id,
        user_content,
        MessageRole::User,
    )
    .await
    .map_err(|e| format!("Failed to save message: {e}"))?;

    ChatsManager::insert_message(db, assistant_msg_id, chat_id, "", MessageRole::Assistant)
        .await
        .map_err(|e| format!("Failed to save message: {e}"))?;

    Ok(())
}

async fn build_api_messages(
    db: &Notitia<AstrumDb, SqliteAdapter>,
    chat_id: &UniqueId,
    system_prompt: &str,
    errors: &Entity<std::collections::VecDeque<String>>,
    cx: &mut AsyncApp,
) -> Option<Box<RawValue>> {
    let messages_result = db
        .query(
            AstrumDb::MESSAGES
                .select((MessageRecord::ROLE, MessageRecord::CONTENT))
                .filter(MessageRecord::CHAT_ID.eq(chat_id.clone()))
                .order_by(MessageRecord::CREATED_AT, OrderDirection::Asc)
                .fetch_all::<BTreeMap<_, _>>(),
        )
        .execute()
        .await;

    let messages_data: BTreeMap<_, (String, String)> = match messages_result {
        Ok(data) => data,
        Err(e) => {
            push_error_async(errors, cx, format!("Failed to load messages: {e}"));
            return None;
        }
    };

    let mut api_messages: Vec<Message> = messages_data
        .values()
        .filter(|(_, content): &&(String, String)| !content.is_empty())
        .map(|(role, content): &(String, String)| Message {
            role: MessageRole::from_str(role),
            content: content.clone(),
        })
        .collect();

    if !system_prompt.is_empty() {
        api_messages.insert(0, Message::system(system_prompt));
    }

    let messages_json = serde_json::to_string(&api_messages).unwrap();
    Some(RawValue::from_string(messages_json).unwrap())
}

async fn stream_chunks(
    provider: Arc<dyn ProviderTrait>,
    model: &str,
    api_messages: Box<RawValue>,
    thinking_enabled: bool,
    chats: &Entity<ChatsManager>,
    assistant_msg_id: &UniqueId,
    cx: &mut AsyncApp,
) {
    let mut options = ChatOptions::new(model).messages_serialized(api_messages);
    if thinking_enabled {
        options = options.thinking(Thinking::Enabled);
    }

    match provider.chat(&options).await {
        Ok(mut response) => {
            while let Some(Ok(chunk)) = response.next().await {
                let _ = chats.update(cx, |chats, cx| {
                    chats.push_chunk(cx, assistant_msg_id, &chunk);
                });
            }
        }
        Err(err) => {
            let _ = chats.update(cx, |chats, cx| {
                chats.push_message_content(cx, assistant_msg_id, &err.to_string());
            });
        }
    };
}
