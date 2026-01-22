use std::{cmp::Reverse, sync::Arc};

use chrono::NaiveDateTime;
use gpui::{App, AppContext, Entity};
use granular_btreemap::GranularBTreeMap;
use rusqlite::Connection;

use crate::managers::{DbError, UniqueId};

mod chat;
pub use chat::*;

type ChatsMap = GranularBTreeMap<UniqueId, Entity<Chat>, Reverse<NaiveDateTime>>;

pub struct ChatsManager {
    db_connection: Option<Arc<Connection>>,
    chats: Entity<Option<ChatsMap>>,
    current_chat_id: Entity<Option<UniqueId>>,
}

impl<'a> ChatsManager {
    pub fn new(cx: &mut App) -> Self {
        Self {
            db_connection: None,
            chats: cx.new(|_cx| None),
            current_chat_id: cx.new(|_cx| None),
        }
    }

    pub fn init(
        &mut self,
        cx: &mut App,
        db_connection: Arc<rusqlite::Connection>,
    ) -> Result<(), DbError> {
        self.db_connection = Some(db_connection.clone());

        db_connection
            .execute_batch(
                "
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS chats (
                    id         TEXT PRIMARY KEY,
                    title      TEXT,
                    created_at DATETIME NOT NULL,
                    edited_at  DATETIME NOT NULL
                );

                CREATE TABLE IF NOT EXISTS messages (
                    id         TEXT PRIMARY KEY,
                    chat_id    TEXT NOT NULL,

                    role       TEXT NOT NULL
                        CHECK (role IN ('system', 'user', 'assistant')),

                    content    TEXT NOT NULL,
                    created_at DATETIME NOT NULL,
                    edited_at  DATETIME NOT NULL,

                    FOREIGN KEY (chat_id)
                        REFERENCES chats(id)
                        ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_messages_chat
                    ON messages(chat_id, created_at);
                ",
            )
            .unwrap();

        let raw_chats = self.load_chats_from_db(cx)?;

        let mut new_chats = GranularBTreeMap::new();
        for raw_chat in raw_chats {
            let edited_at = raw_chat.edited_at.clone();

            new_chats.insert(
                raw_chat.chat_id.clone(),
                cx.new(|_cx| raw_chat),
                Reverse(edited_at),
            );
        }

        self.chats.update(cx, |chats, _cx| {
            *chats = Some(new_chats);
        });

        Ok(())
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

    pub fn get_current_chat(&'a self, cx: &'a mut App) -> Result<Option<Entity<Chat>>, DbError> {
        let db_connection = self
            .db_connection
            .as_ref()
            .ok_or_else(|| DbError::MissingData("database connection"))?;

        let Some(current_chat_id) = self.current_chat_id.read(cx).as_ref().cloned() else {
            return Ok(None);
        };

        self.chats.update(cx, |chats, cx| {
            let chats = chats
                .as_mut()
                .ok_or_else(|| DbError::MissingData("chats"))?;

            match chats.get(&current_chat_id) {
                Some(chat) => Ok(Some(chat.clone())),
                None => {
                    let chat = Chat::load_from_db(
                        cx,
                        db_connection.clone(),
                        current_chat_id.clone(),
                        self.chats.clone(),
                    )
                    .map_err(|err| DbError::SqliteError(err))?;

                    let edited_at = chat.edited_at.clone();

                    let chat = cx.new(|_cx| chat);
                    chats.insert(current_chat_id, chat.clone(), Reverse(edited_at));
                    Ok(Some(chat))
                }
            }
        })
    }

    pub fn create_chat(&self, cx: &mut App) -> Result<Entity<Chat>, DbError> {
        let db_connection = self
            .db_connection
            .as_ref()
            .ok_or_else(|| DbError::MissingData("database connection"))?;

        let chat = Chat::new(cx, db_connection.clone(), self.chats.clone())
            .map_err(|err| DbError::SqliteError(err))?;
        let chat_id = chat.chat_id.clone();
        let edited_at = chat.edited_at.clone();
        let chat = cx.new(|_cx| chat);

        self.chats.update(cx, |chats, cx| {
            let chats = chats.get_or_insert_default();
            chats.insert(chat_id, chat.clone(), Reverse(edited_at));
            cx.notify();
        });

        Ok(chat)
    }

    pub fn chats_iter(&'a self, cx: &'a App) -> Option<impl Iterator<Item = &'a Chat>> {
        self.chats
            .read(cx)
            .as_ref()
            .map(|chats| chats.values().map(|chat| chat.read(cx)))
    }

    fn load_chats_from_db(&'a self, cx: &mut App) -> Result<Box<[Chat]>, DbError> {
        let db_connection = self
            .db_connection
            .as_ref()
            .ok_or_else(|| DbError::MissingData("database connection"))?
            .clone();

        let mut stmt = db_connection
            .prepare(
                r#"
                SELECT
                    id
                FROM chats
                ORDER BY edited_at ASC
                "#,
            )
            .map_err(|err| DbError::SqliteError(err))?;

        stmt.query_map([], |row| {
            Chat::load_from_db(
                cx,
                db_connection.clone(),
                UniqueId::from_string(row.get::<_, String>(0)?),
                self.chats.clone(),
            )
        })
        .map_err(|err| DbError::SqliteError(err))?
        .collect::<rusqlite::Result<Box<[Chat]>>>()
        .map_err(|err| DbError::SqliteError(err))
    }
}
