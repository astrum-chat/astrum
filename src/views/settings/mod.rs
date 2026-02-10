use std::sync::Arc;

use gpui::{Context, ElementId, IntoElement, Render, Window};
use gpui_tesserae::ElementIdExt;
use smol::lock::RwLock;

use crate::{managers::Managers, views::BaseView, views::settings::blocks::SettingsArea};

mod blocks;
use blocks::Sidebar;

pub struct SettingsView {
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
}

impl SettingsView {
    pub fn new(id: impl Into<ElementId>, managers: Arc<RwLock<Managers>>) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        BaseView::new()
            .docked_left(Sidebar::new(
                self.id.with_suffix("sidebar"),
                self.managers.clone(),
            ))
            .child(SettingsArea::new(
                self.id.with_suffix("settings_area"),
                self.managers.clone(),
            ))
    }
}
