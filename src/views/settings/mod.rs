use gpui::{App, AppContext, Context, ElementId, Entity, IntoElement, Render, SharedString, Window};
use gpui_tesserae::ElementIdExt;

use crate::{managers::Managers, views::BaseView, views::settings::blocks::SettingsArea};

mod blocks;
use blocks::Sidebar;

pub struct SettingsView {
    id: ElementId,
    managers: Managers,
    settings_page: Entity<SharedString>,
}

impl SettingsView {
    pub fn new(id: impl Into<ElementId>, managers: Managers, cx: &mut App) -> Self {
        Self {
            id: id.into(),
            managers,
            settings_page: cx.new(|_cx| SharedString::new("Providers")),
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        BaseView::new()
            .docked_left(Sidebar::new(
                self.id.with_suffix("sidebar"),
                self.settings_page.clone(),
            ))
            .child(SettingsArea::new(
                self.id.with_suffix("settings_area"),
                self.managers.clone(),
                self.settings_page.clone(),
            ))
    }
}
