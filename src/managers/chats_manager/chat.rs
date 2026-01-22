use std::{cmp::Reverse, sync::Arc};

use anyml::{Message, MessageRole};
use chrono::{NaiveDateTime, Utc};
use gpui::{App, AppContext, Entity};
use indexmap::IndexMap;
use rusqlite::Connection;
use serde::{Serialize, Serializer, ser::SerializeSeq};

use crate::managers::{UniqueId, chats_manager::ChatsMap};

pub struct Chat {
    db_connection: Arc<Connection>,
    pub chat_id: UniqueId,
    pub title: Entity<String>,
    pub edited_at: NaiveDateTime,
    messages: Entity<IndexMap<UniqueId, MessageWithMetadata>>,
    chats: Entity<Option<ChatsMap>>,
}

#[derive(Serialize)]
pub struct MessageWithMetadata {
    #[serde(flatten)]
    pub message: Message,
    #[serde(skip)]
    message_id: UniqueId,
}

impl<'a> Chat {
    pub fn load_from_db(
        cx: &mut App,
        db_connection: Arc<Connection>,
        chat_id: UniqueId,
        chats: Entity<Option<ChatsMap>>,
    ) -> rusqlite::Result<Self> {
        let mut stmt = db_connection.prepare(
            r#"
                SELECT
                    title,
                    edited_at
                FROM chats
                WHERE id = ?
                "#,
        )?;

        let (title, edited_at) = stmt.query_row([chat_id.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, NaiveDateTime>(1)?))
        })?;

        Ok(Chat {
            db_connection: db_connection.clone(),
            title: cx.new(|_cx| title),
            edited_at,
            messages: {
                let messages = Self::load_messages_from_db(&chat_id, &db_connection)?;
                cx.new(|_cx| messages)
            },
            chat_id,
            chats,
        })
    }

    pub fn new(
        cx: &mut App,
        db_connection: Arc<Connection>,
        chats: Entity<Option<ChatsMap>>,
    ) -> rusqlite::Result<Self> {
        let chat_id = UniqueId::new();
        let created_at = Utc::now().naive_utc();

        db_connection.execute(
            "INSERT INTO chats (id, title, created_at, edited_at) VALUES (?1, ?2, ?3, ?3)",
            (&chat_id, "Untitled Chat", &created_at),
        )?;

        Ok(Self {
            db_connection,
            chat_id,
            edited_at: created_at,
            title: cx.new(|_cx| String::from("Untitled Chat")),
            messages: cx.new(|_cx| IndexMap::new()),
            chats,
        })
    }

    pub fn read_messages(&'a self, cx: &'a App) -> &'a IndexMap<UniqueId, MessageWithMetadata> {
        self.messages.read(cx)
    }

    pub fn push_message(
        &mut self,
        cx: &mut App,
        chat_id: &UniqueId,
        content: impl Into<String>,
        role: MessageRole,
    ) -> Result<UniqueId, rusqlite::Error> {
        let content = content.into();

        let message_id = UniqueId::new();
        let created_at = Utc::now().naive_utc();

        self.db_connection.execute(
            "INSERT INTO messages (id, chat_id, role, content, created_at, edited_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            (&message_id, chat_id, role.as_str(), &content, &created_at),
        )?;

        // Pushes the message to our cache.
        self.messages.update(cx, |messages, cx| {
            messages.insert(
                message_id.clone(),
                MessageWithMetadata {
                    message: Message { content, role },
                    message_id: message_id.clone(),
                },
            );
            cx.notify();
        });

        // Updates our cached chats map with the created_at time stamp.
        self.chats.update(cx, |chats, cx| {
            let Some(chats) = chats else { return };
            chats
                .update_order_for_key(&self.chat_id, Reverse(created_at))
                .unwrap();

            cx.notify();
        });

        Ok(message_id)
    }

    pub fn push_message_content(
        &self,
        cx: &mut App,
        message_id: &UniqueId,
        content: impl Into<String>,
    ) -> Result<(), rusqlite::Error> {
        let edited_at = Utc::now().naive_utc();

        let content = content.into();

        // Appends the content to the cached message.
        self.messages.update(cx, |chat, cx| {
            let Some(message) = chat.get_mut(message_id) else {
                return;
            };
            message.message.content += &content;
            cx.notify();
        });

        let new_cached_content = self
            .messages
            .read(cx)
            .get(message_id)
            .map(|this| &this.message.content);

        // Appends the content to the message in the database.
        let new_content: String = self.db_connection.query_row(
            r#"
            UPDATE messages
            SET
                content = content || ?2,
                edited_at = ?3
            WHERE id = ?1
            RETURNING content
            "#,
            (&message_id, &content, &edited_at),
            |row| row.get(0),
        )?;

        // We need to resolve the desync using the database as our primary source.
        if Some(&new_content) != new_cached_content {
            self.messages.update(cx, |chat, cx| {
                let Some(message) = chat.get_mut(message_id) else {
                    return;
                };
                message.message.content = new_content;
                cx.notify();
            });
        }

        // We need to update our internal chats map with the new edited_at time stamp.
        self.chats.update(cx, |chats, cx| {
            let Some(chats) = chats else { return };
            chats
                .update_order_for_key(&self.chat_id, Reverse(edited_at))
                .unwrap();

            cx.notify();
        });

        Ok(())
    }

    fn load_messages_from_db(
        message_id: &UniqueId,
        db_connection: &Connection,
    ) -> rusqlite::Result<IndexMap<UniqueId, MessageWithMetadata>> {
        let mut stmt = db_connection.prepare(
            r#"
            SELECT
                id,
                content,
                role
            FROM messages
            WHERE chat_id = ?
            ORDER BY edited_at ASC
            "#,
        )?;

        let messages = stmt
            .query_map([message_id.to_string()], |row| {
                let message_id = UniqueId::from_string(row.get::<_, String>(0)?);
                let content: String = row.get(1)?;
                let role: String = row.get(2)?;

                Ok((
                    message_id.clone(),
                    MessageWithMetadata {
                        message: anyml::Message {
                            content,
                            role: MessageRole::from_str(&role),
                        },
                        message_id,
                    },
                ))
            })?
            .collect::<rusqlite::Result<IndexMap<_, _>>>()?;

        Ok(messages)
    }
}

pub struct ValuesOnly<'a, K, V>(pub &'a IndexMap<K, V>);

impl<'a, K, V> Serialize for ValuesOnly<'a, K, V>
where
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for value in self.0.values() {
            seq.serialize_element(value)?;
        }
        seq.end()
    }
}
