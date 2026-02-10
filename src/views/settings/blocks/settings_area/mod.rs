use std::sync::Arc;

use gpui::{App, ElementId, IntoElement, RenderOnce, div, prelude::*, px};
use smol::lock::RwLock;

use crate::{Managers, views::settings::blocks::settings_area::pages::render_settings_page};

mod pages;

#[derive(IntoElement)]
pub struct SettingsArea {
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
}

impl SettingsArea {
    pub fn new(id: impl Into<ElementId>, managers: Arc<RwLock<Managers>>) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl RenderOnce for SettingsArea {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        div()
            .id(self.id.clone())
            .size_full()
            .flex()
            .justify_center()
            .p(px(20.))
            .pb(px(0.))
            .child(render_settings_page(cx, self.id, self.managers))
    }
}
