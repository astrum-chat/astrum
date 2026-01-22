use std::path::PathBuf;

pub struct PersistenceManager {}

const BASE_DIR_NAME: &str = "chat.astrum.astrum";

impl PersistenceManager {
    pub fn new() -> Self {
        Self {}
    }

    pub fn config_dir(&self) -> Option<PathBuf> {
        dirs::config_dir().map(|this| this.join(BASE_DIR_NAME))
    }

    pub fn local_config_dir(&self) -> Option<PathBuf> {
        dirs::config_local_dir().map(|this| this.join(BASE_DIR_NAME))
    }

    pub fn data_dir(&self) -> Option<PathBuf> {
        dirs::data_dir().map(|this| this.join(BASE_DIR_NAME))
    }

    pub fn local_data_dir(&self) -> Option<PathBuf> {
        dirs::data_local_dir().map(|this| this.join(BASE_DIR_NAME))
    }
}
