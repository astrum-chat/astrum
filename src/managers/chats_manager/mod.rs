use std::collections::HashMap;

use anyml::MessageRole;
use futures::future::AbortHandle;
use gpui::{App, AppContext, Entity};
use notitia::Notitia;
use notitia_sqlite::SqliteAdapter;
use smol::channel::Sender;

use schema::{AstrumDb, ChatRecord, DbDateTime, MessageRecord, UniqueId};

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
                            db.mutate(
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
                            .unwrap();
                        }
                        MessageMutation::SetTitle { title } => {
                            db.mutate(
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
                            .unwrap();
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

    /// Create a new chat and return its ID.
    pub fn create_chat(&self, cx: &mut App) -> UniqueId {
        let chat_id = UniqueId::new();
        let now = DbDateTime::now();
        let db = self.db().clone();
        let chat_id_clone = chat_id.clone();
        smol::block_on(async {
            db.mutate(
                AstrumDb::CHATS.insert(
                    ChatRecord::build()
                        .id(chat_id_clone)
                        .title("Untitled Chat")
                        .created_at(now.clone())
                        .edited_at(now),
                ),
            )
            .execute()
            .await
            .unwrap();
        });
        chat_id
    }

    /// Insert a message and bump the chat's `edited_at`.
    pub fn push_message(
        &self,
        chat_id: &UniqueId,
        content: impl Into<String>,
        role: MessageRole,
    ) -> UniqueId {
        let msg_id = UniqueId::new();
        let now = DbDateTime::now();
        let db = self.db().clone();
        let chat_id = chat_id.clone();
        let content = content.into();
        let msg_id_clone = msg_id.clone();
        smol::block_on(async {
            db.mutate(
                AstrumDb::MESSAGES.insert(
                    MessageRecord::build()
                        .id(msg_id_clone)
                        .chat_id(chat_id.clone())
                        .role(role.as_str())
                        .content(content)
                        .created_at(now.clone())
                        .edited_at(now.clone()),
                ),
            )
            .execute()
            .await
            .unwrap();
            // Bump chat's edited_at
            db.mutate(
                AstrumDb::CHATS
                    .update(ChatRecord::build().edited_at(now))
                    .filter(ChatRecord::ID.eq(chat_id)),
            )
            .execute()
            .await
            .unwrap();
        });
        msg_id
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
    pub fn delete_chat(&mut self, cx: &mut App, chat_id: UniqueId) {
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
        cx.spawn(async move |_cx| {
            db.mutate(
                AstrumDb::CHATS
                    .delete()
                    .filter(ChatRecord::ID.eq(chat_id)),
            )
            .execute()
            .await
            .unwrap();
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
