mod send_message;

use anyml::{ChatChunk, ChatOptions, MessageRole, models::Message};
use futures::future::{AbortHandle, Abortable};
use gpui::{App, AsyncApp, SharedString};

use schema::UniqueId;

use crate::managers::{ChatsManager, Managers};
use crate::utils::errors::push_error_async;

pub use send_message::send_message;

fn create_thread_title(
    managers: &Managers,
    chat_id: &UniqueId,
    contents: &SharedString,
    cx: &mut App,
) {
    let Some((chat_titles_provider, chat_titles_model)) =
        managers.models.read_with(cx, |models, cx| {
            models
                .get_chat_titles_provider(cx)
                .zip(models.get_chat_titles_model(cx))
        })
    else {
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
                "Summarize this into a short 4-6 word thread title. DO NOT INCLUDE ANY PUNCTUATION - ESPECIALLY SINGLE OR DOUBLE QUOTES. Keep it natural and concise.\n\nMessage: \"{}\"\nTitle:",
                user_message
            );

            let messages = [Message {
                content: prompt,
                role: MessageRole::User,
            }];
            let options = ChatOptions::new(&chat_titles_model).messages(&messages);

            match chat_titles_provider.inner.chat(&options).await {
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
