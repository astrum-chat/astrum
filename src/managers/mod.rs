use gpui::{App, AppContext, Entity};
use notitia::Notitia;
use notitia_sqlite::SqliteAdapter;

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
/// Each manager is a GPUI `Entity<T>`, providing built-in interior
/// mutability, change tracking, and reactivity with no manual locking.
#[derive(Clone)]
pub struct Managers {
    pub models: Entity<ModelsManager>,
    pub chats: Entity<ChatsManager>,
    pub persistence: PersistenceManager,
    pub available_update: Entity<Option<ReleaseInfo>>,
}

impl Managers {
    pub fn new(cx: &mut App) -> Self {
        let update = UpdateManager::new(cx);

        Self {
            models: cx.new(|cx| ModelsManager::new(cx)),
            chats: cx.new(|cx| ChatsManager::new(cx)),
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
        self.models.update(cx, |models, cx| models.init(cx, db.clone()));
        self.chats.update(cx, |chats, cx| chats.init(cx, db));
    }
}
