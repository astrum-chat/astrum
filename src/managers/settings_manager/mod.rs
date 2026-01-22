use gpui::{App, AppContext, Entity, SharedString};

pub struct SettingsManager {
    pub current_settings_page_name: Entity<SharedString>,
}

impl SettingsManager {
    pub fn new(cx: &mut App) -> Self {
        Self {
            current_settings_page_name: cx.new(|_cx| SharedString::new("Providers")),
        }
    }
}
