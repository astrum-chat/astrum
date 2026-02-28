use std::collections::{HashMap, VecDeque};

use anyml::MessageRole;
use futures::future::AbortHandle;
use gpui::{App, AppContext, Entity};
use notitia::Notitia;
use notitia_sqlite::SqliteAdapter;
use smol::channel::Sender;
use tracing::error;

use schema::{AstrumDb, ChatRecord, DbDateTime, MessageRecord, UniqueId};

use crate::utils::errors::push_error_async;

/// A queued mutation to be executed sequentially per message.
enum MessageMutation {
    AppendContent { chunk: String },
    SetTitle { title: String },
}

pub struct ChatsManager {
    db: Option<Notitia<AstrumDb, SqliteAdapter>>,
    current_chat_id: Entity<Option<UniqueId>>,
    pub is_streaming: Entity<bool>,
    pub streaming_abort_handle: Entity<Option<AbortHandle>>,
    mutation_queues: HashMap<UniqueId, Sender<MessageMutation>>,
}

impl ChatsManager {
    pub fn new(cx: &mut App) -> Self {
        Self {
            db: None,
            current_chat_id: cx.new(|_cx| None),
            is_streaming: cx.new(|_cx| false),
            streaming_abort_handle: cx.new(|_cx| None),
            mutation_queues: HashMap::new(),
        }
    }

    pub fn init(&mut self, _cx: &mut App, db: Notitia<AstrumDb, SqliteAdapter>) {
        self.db = Some(db);
    }

    fn queue_for(&mut self, id: &UniqueId, cx: &mut App) -> &Sender<MessageMutation> {
        let db = self.db().clone();
        let id_clone = id.clone();
        self.mutation_queues.entry(id.clone()).or_insert_with(|| {
            let (tx, rx) = smol::channel::unbounded::<MessageMutation>();
            cx.spawn(async move |_cx| {
                while let Ok(task) = rx.recv().await {
                    match task {
                        MessageMutation::AppendContent { chunk } => {
                            if let Err(e) = db
                                .mutate(
                                    AstrumDb::MESSAGES
                                        .update(
                                            MessageRecord::build()
                                                .content(MessageRecord::CONTENT.concat(chunk))
                                                .edited_at(DbDateTime::now()),
                                        )
                                        .filter(MessageRecord::ID.eq(id_clone.clone())),
                                )
                                .execute()
                                .await
                            {
                                error!("Failed to append message content: {e}");
                            }
                        }
                        MessageMutation::SetTitle { title } => {
                            if let Err(e) = db
                                .mutate(
                                    AstrumDb::CHATS
                                        .update(
                                            ChatRecord::build()
                                                .title(title)
                                                .edited_at(DbDateTime::now()),
                                        )
                                        .filter(ChatRecord::ID.eq(id_clone.clone())),
                                )
                                .execute()
                                .await
                            {
                                error!("Failed to update chat title: {e}");
                            }
                        }
                    }
                }
            })
            .detach();
            tx
        })
    }

    pub fn drop_mutation_queue(&mut self, id: &UniqueId) {
        self.mutation_queues.remove(id);
    }

    pub fn db(&self) -> &Notitia<AstrumDb, SqliteAdapter> {
        self.db.as_ref().expect("ChatsManager not initialized")
    }

    pub fn db_initialized(&self) -> bool {
        self.db.is_some()
    }

    pub fn get_current_chat_id(&self) -> &Entity<Option<UniqueId>> {
        &self.current_chat_id
    }

    pub fn set_current_chat(&self, cx: &mut App, chat_id: UniqueId) {
        self.current_chat_id.update(cx, |current_chat_id, cx| {
            *current_chat_id = Some(chat_id);
            cx.notify();
        });
    }

    pub fn set_streaming(&self, cx: &mut App, streaming: bool) {
        self.is_streaming.update(cx, |is_streaming, cx| {
            *is_streaming = streaming;
            cx.notify();
        });
    }

    pub fn set_abort_handle(&self, cx: &mut App, handle: Option<AbortHandle>) {
        self.streaming_abort_handle.update(cx, |abort_handle, cx| {
            *abort_handle = handle;
            cx.notify();
        });
    }

    pub fn cancel_streaming(&self, cx: &mut App) {
        if let Some(handle) = self.streaming_abort_handle.read(cx).as_ref() {
            handle.abort();
        }
        self.set_abort_handle(cx, None);
        self.set_streaming(cx, false);
    }

    /// Insert a new chat row into the DB.
    pub async fn insert_chat(
        db: &Notitia<AstrumDb, SqliteAdapter>,
        chat_id: &UniqueId,
    ) -> anyhow::Result<()> {
        let now = DbDateTime::now();
        db.mutate(
            AstrumDb::CHATS.insert(
                ChatRecord::build()
                    .id(chat_id.clone())
                    .title("Untitled Chat")
                    .created_at(now.clone())
                    .edited_at(now),
            ),
        )
        .execute()
        .await?;
        Ok(())
    }

    /// Insert a message row and bump the chat's `edited_at`.
    pub async fn insert_message(
        db: &Notitia<AstrumDb, SqliteAdapter>,
        msg_id: &UniqueId,
        chat_id: &UniqueId,
        content: &str,
        role: MessageRole,
    ) -> anyhow::Result<()> {
        let now = DbDateTime::now();
        db.mutate(
            AstrumDb::MESSAGES.insert(
                MessageRecord::build()
                    .id(msg_id.clone())
                    .chat_id(chat_id.clone())
                    .role(role.as_str())
                    .content(content)
                    .created_at(now.clone())
                    .edited_at(now.clone()),
            ),
        )
        .execute()
        .await?;
        db.mutate(
            AstrumDb::CHATS
                .update(ChatRecord::build().edited_at(now))
                .filter(ChatRecord::ID.eq(chat_id.clone())),
        )
        .execute()
        .await?;
        Ok(())
    }

    pub fn push_message_content(
        &mut self,
        cx: &mut App,
        message_id: &UniqueId,
        chunk: impl Into<String>,
    ) {
        let queue = self.queue_for(message_id, cx);
        let _ = queue.send_blocking(MessageMutation::AppendContent {
            chunk: chunk.into(),
        });
    }

    /// Delete a chat and all its messages (cascade).
    pub fn delete_chat(
        &mut self,
        cx: &mut App,
        chat_id: UniqueId,
        errors: Entity<VecDeque<String>>,
    ) {
        // If deleting the current chat, cancel any active stream and clear selection
        if self.current_chat_id.read(cx).as_ref() == Some(&chat_id) {
            if *self.is_streaming.read(cx) {
                self.cancel_streaming(cx);
            }
            self.current_chat_id.update(cx, |id, cx| {
                *id = None;
                cx.notify();
            });
        }

        self.drop_mutation_queue(&chat_id);

        // Async delete from DB (messages cascade-delete via schema)
        let db = self.db().clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            if let Err(e) = db
                .mutate(
                    AstrumDb::CHATS
                        .delete()
                        .filter(ChatRecord::ID.eq(chat_id)),
                )
                .execute()
                .await
            {
                push_error_async(&errors, cx, format!("Failed to delete chat: {e}"));
            }
        })
        .detach();
    }

    pub fn set_title(&mut self, cx: &mut App, chat_id: &UniqueId, title: impl Into<String>) {
        let queue = self.queue_for(chat_id, cx);
        let _ = queue.send_blocking(MessageMutation::SetTitle {
            title: title.into(),
        });
    }
}
