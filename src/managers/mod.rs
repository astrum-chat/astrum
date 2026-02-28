use std::sync::Arc;

use gpui::{App, Entity};
use notitia::Notitia;
use notitia_sqlite::SqliteAdapter;
use smol::lock::RwLock;

use schema::AstrumDb;

mod models_manager;
pub use models_manager::*;

mod chats_manager;
pub use chats_manager::*;

mod persistence_manager;
pub use persistence_manager::*;

mod settings_manager;
pub use settings_manager::*;

mod update_manager;
pub use update_manager::*;

/// Application-wide manager handles.
///
/// Each manager is independently lockable, eliminating the single-lock
/// contention of the old `Arc<RwLock<Managers>>` god object.
///
/// Lightweight entities (settings page name, available update) are stored
/// as bare `Entity<T>` values — no lock needed since GPUI entities have
/// built-in interior mutability and change tracking.
#[derive(Clone)]
pub struct Managers {
    pub models: Arc<RwLock<ModelsManager>>,
    pub chats: Arc<RwLock<ChatsManager>>,
    pub persistence: PersistenceManager,
    pub available_update: Entity<Option<ReleaseInfo>>,
}

impl Managers {
    pub fn new(cx: &mut App) -> Self {
        let update = UpdateManager::new(cx);

        Self {
            models: Arc::new(RwLock::new(ModelsManager::new(cx))),
            chats: Arc::new(RwLock::new(ChatsManager::new(cx))),
            persistence: PersistenceManager::new(),
            available_update: update.available_update,
        }
    }

    /// Get the database connection URL. Must be called before `connect_db`.
    pub fn db_url(&self) -> String {
        let db_path = self.persistence.local_data_dir().unwrap().join("db.sqlite");
        format!("sqlite:{}", db_path.display())
    }

    /// Initialize managers with an already-connected database.
    /// Call this inside `cx.update()` after awaiting `connect_db`.
    pub fn init_with_db(&self, cx: &mut App, db: Notitia<AstrumDb, SqliteAdapter>) {
        self.models.write_blocking().init(cx, db.clone());
        self.chats.write_blocking().init(cx, db);
    }
}
