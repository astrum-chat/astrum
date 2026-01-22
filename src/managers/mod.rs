mod unique_id;

use std::sync::Arc;

use thiserror::Error;

use gpui::App;
pub use unique_id::*;

mod models_manager;
pub use models_manager::*;

mod chats_manager;
pub use chats_manager::*;

mod persistence_manager;
pub use persistence_manager::*;

mod settings_manager;
pub use settings_manager::*;

pub struct Managers {
    pub models: ModelsManager,
    pub chats: ChatsManager,
    pub persistence: PersistenceManager,
    pub settings: SettingsManager,
}

impl Managers {
    pub fn new(cx: &mut App) -> Self {
        Self {
            models: ModelsManager::new(cx),
            chats: ChatsManager::new(cx),
            persistence: PersistenceManager::new(),
            settings: SettingsManager::new(cx),
        }
    }

    pub fn init(&mut self, cx: &mut App) -> rusqlite::Result<()> {
        let db_dir = self.persistence.local_data_dir().unwrap().join("db.sqlite");

        let db_connection = Arc::new(rusqlite::Connection::open(db_dir)?);

        self.models.init(cx, db_connection.clone());
        self.chats.init(cx, db_connection).unwrap();

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Missing data: {0}")]
    MissingData(&'static str),

    #[error("error: {0}")]
    Error(&'static str),

    #[error("An error with sqlite.")]
    SqliteError(#[source] rusqlite::Error),
}
