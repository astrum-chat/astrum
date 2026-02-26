use notitia::prelude::*;

mod unique_id;
pub use unique_id::UniqueId;

mod db_datetime;
pub use db_datetime::*;

#[database]
pub struct AstrumDb {
    pub chats: Table<ChatRecord>,
    #[db(foreign_key(chat_id, chats.id, on_delete = Cascade))]
    pub messages: Table<MessageRecord>,
    pub providers: Table<ProviderRecord>,
    pub model_selections: Table<ModelSelectionRecord>,
}

#[record]
pub struct ChatRecord {
    #[db(primary_key)]
    pub id: UniqueId,
    pub title: Option<String>,
    pub created_at: DbDateTime,
    pub edited_at: DbDateTime,
}

#[record]
pub struct MessageRecord {
    #[db(primary_key)]
    pub id: UniqueId,
    pub chat_id: UniqueId,
    pub role: String,
    pub content: String,
    pub created_at: DbDateTime,
    pub edited_at: DbDateTime,
}

#[record(removed_fields(icon))]
pub struct ProviderRecord {
    #[db(primary_key)]
    pub id: UniqueId,
    pub kind: String,
    pub name: String,
    pub url: Option<String>,
    pub created_at: Option<DbDateTime>,
    pub edited_at: Option<DbDateTime>,
}

#[record]
pub struct ModelSelectionRecord {
    #[db(primary_key)]
    pub key: String,
    pub provider_id: Option<UniqueId>,
    pub provider_name: Option<String>,
    pub model: Option<String>,
    pub parameters: Option<String>,
    pub quantization: Option<String>,
}

pub mod schemas {
    pub use super::AstrumDb;
}
