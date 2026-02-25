use gpui::App;
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

pub struct Managers {
    pub db: Option<Notitia<AstrumDb, SqliteAdapter>>,
    pub models: ModelsManager,
    pub chats: ChatsManager,
    pub persistence: PersistenceManager,
    pub settings: SettingsManager,
    pub update: UpdateManager,
}

impl Managers {
    pub fn new(cx: &mut App) -> Self {
        Self {
            db: None,
            models: ModelsManager::new(cx),
            chats: ChatsManager::new(cx),
            persistence: PersistenceManager::new(),
            settings: SettingsManager::new(cx),
            update: UpdateManager::new(cx),
        }
    }

    /// Get the database connection URL. Must be called before `connect_db`.
    pub fn db_url(&self) -> String {
        let db_path = self.persistence.local_data_dir().unwrap().join("db.sqlite");
        format!("sqlite:{}", db_path.display())
    }

    /// Initialize managers with an already-connected database.
    /// Call this inside `cx.update()` after awaiting `connect_db`.
    pub fn init_with_db(&mut self, cx: &mut App, db: Notitia<AstrumDb, SqliteAdapter>) {
        self.db = Some(db.clone());
        self.models.init(cx, db.clone());
        self.chats.init(cx, db);
    }
}
