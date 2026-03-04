use std::collections::{HashMap, VecDeque};

use anyml::{ChatChunk, MessageRole};
use futures::future::AbortHandle;
use gpui::{App, AppContext, Entity};
use notitia::Notitia;
use notitia_sqlite::SqliteAdapter;
use smol::channel::Sender;
use tracing::error;

use schema::{AstrumDb, ChatRecord, DbDateTime, MessageRecord, UniqueId};

use crate::utils::errors::push_error_async;

/// Delimiter used to separate thinking blocks from content in the DB.
pub const THINK_DELIMITER: &str = "\n<|think|>\n";

/// Tracks the kind of the last streamed chunk for a message.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ChatChunkKind {
    Content,
    Thinking,
}

/// A queued mutation to be executed sequentially per message.
enum MessageMutation {
    AppendContent { chunk: String },
    SetTitle { title: String },
}

pub struct ChatsManager {
    db: Option<Notitia<AstrumDb, SqliteAdapter>>,
    current_chat_id: Entity<Option<UniqueId>>,
    pub is_streaming: Entity<bool>,
    pub thinking_enabled: Entity<bool>,
    mutation_queues: HashMap<UniqueId, Sender<MessageMutation>>,
    title_abort_handles: HashMap<UniqueId, AbortHandle>,
    /// Tracks the kind of the last streamed chunk for each message.
    last_chunk_kind: HashMap<UniqueId, ChatChunkKind>,
}

impl ChatsManager {
    pub fn new(cx: &mut App) -> Self {
        Self {
            db: None,
            current_chat_id: cx.new(|_cx| None),
            is_streaming: cx.new(|_cx| false),
            thinking_enabled: cx.new(|_cx| false),
            mutation_queues: HashMap::new(),
            title_abort_handles: HashMap::new(),
            last_chunk_kind: HashMap::new(),
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
        self.last_chunk_kind.remove(id);
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

    pub fn cancel_streaming(&self, cx: &mut App) {
        self.set_streaming(cx, false);
    }

    pub fn set_title_abort_handle(&mut self, chat_id: UniqueId, handle: AbortHandle) {
        self.title_abort_handles.insert(chat_id, handle);
    }

    pub fn cancel_title_generation(&mut self, chat_id: &UniqueId) {
        if let Some(handle) = self.title_abort_handles.remove(chat_id) {
            handle.abort();
        }
    }

    pub fn clear_title_abort_handle(&mut self, chat_id: &UniqueId) {
        self.title_abort_handles.remove(chat_id);
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

    /// Pushes a streaming chunk, inserting `<|think|>` delimiters on type transitions.
    pub fn push_chunk(
        &mut self,
        cx: &mut App,
        message_id: &UniqueId,
        chunk: &ChatChunk,
    ) {
        let (kind, text) = match chunk {
            ChatChunk::Content(t) => (ChatChunkKind::Content, t.as_str()),
            ChatChunk::Thinking(t) => (ChatChunkKind::Thinking, t.as_str()),
        };

        let last = self.last_chunk_kind.get(message_id).copied();
        let needs_delimiter = last != Some(kind) && (last.is_some() || kind == ChatChunkKind::Thinking);

        self.last_chunk_kind.insert(message_id.clone(), kind);

        if needs_delimiter {
            let mut combined = String::with_capacity(THINK_DELIMITER.len() + text.len());
            combined.push_str(THINK_DELIMITER);
            combined.push_str(text);
            self.push_message_content(cx, message_id, combined);
        } else {
            self.push_message_content(cx, message_id, text);
        }
    }

    pub fn toggle_thinking(&self, cx: &mut App) {
        self.thinking_enabled.update(cx, |enabled, cx| {
            *enabled = !*enabled;
            cx.notify();
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

        self.cancel_title_generation(&chat_id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use notitia::prelude::*;
    use notitia::PrimaryKey;

    async fn test_db() -> Notitia<AstrumDb, SqliteAdapter> {
        AstrumDb::connect::<SqliteAdapter>("sqlite::memory:")
            .await
            .expect("Failed to create in-memory test DB")
    }

    #[test]
    fn test_insert_chat_creates_row() {
        smol::block_on(async {
            let db = test_db().await;
            let chat_id = UniqueId::new();

            ChatsManager::insert_chat(&db, &chat_id).await.unwrap();

            let result: Result<(PrimaryKey<UniqueId>, Option<String>), _> = db
                .query(
                    AstrumDb::CHATS
                        .select((ChatRecord::ID, ChatRecord::TITLE))
                        .filter(ChatRecord::ID.eq(chat_id.clone()))
                        .fetch_one(),
                )
                .execute()
                .await;
            let (pk, title) = result.unwrap();
            assert_eq!(*pk, chat_id);
            assert_eq!(title, Some("Untitled Chat".to_string()));
        });
    }

    #[test]
    fn test_insert_message_creates_row() {
        smol::block_on(async {
            let db = test_db().await;
            let chat_id = UniqueId::new();
            let msg_id = UniqueId::new();

            ChatsManager::insert_chat(&db, &chat_id).await.unwrap();
            ChatsManager::insert_message(&db, &msg_id, &chat_id, "Hello!", MessageRole::User)
                .await
                .unwrap();

            let result: Result<(PrimaryKey<UniqueId>, String), _> = db
                .query(
                    AstrumDb::MESSAGES
                        .select((MessageRecord::ID, MessageRecord::CONTENT))
                        .filter(MessageRecord::ID.eq(msg_id.clone()))
                        .fetch_one(),
                )
                .execute()
                .await;
            let (pk, content) = result.unwrap();
            assert_eq!(*pk, msg_id);
            assert_eq!(content, "Hello!");
        });
    }

    #[test]
    fn test_insert_multiple_messages_different_roles() {
        smol::block_on(async {
            let db = test_db().await;
            let chat_id = UniqueId::new();
            ChatsManager::insert_chat(&db, &chat_id).await.unwrap();

            let user_msg = UniqueId::new();
            let assistant_msg = UniqueId::new();

            ChatsManager::insert_message(&db, &user_msg, &chat_id, "question", MessageRole::User)
                .await
                .unwrap();
            ChatsManager::insert_message(&db, &assistant_msg, &chat_id, "answer", MessageRole::Assistant)
                .await
                .unwrap();

            let result: Result<Vec<(PrimaryKey<UniqueId>, String, String)>, _> = db
                .query(
                    AstrumDb::MESSAGES
                        .select((MessageRecord::ID, MessageRecord::ROLE, MessageRecord::CONTENT))
                        .filter(MessageRecord::CHAT_ID.eq(chat_id))
                        .fetch_all::<Vec<_>>(),
                )
                .execute()
                .await;
            let messages = result.unwrap();
            assert_eq!(messages.len(), 2);
        });
    }

    #[test]
    fn test_insert_chat_duplicate_id_fails() {
        smol::block_on(async {
            let db = test_db().await;
            let chat_id = UniqueId::new();

            ChatsManager::insert_chat(&db, &chat_id).await.unwrap();
            let result = ChatsManager::insert_chat(&db, &chat_id).await;
            assert!(result.is_err());
        });
    }
}
