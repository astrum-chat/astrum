use gpui::{App, ElementId, Entity, IntoElement, RenderOnce, SharedString, div, prelude::*, px};

use crate::{managers::Managers, views::settings::blocks::settings_area::pages::render_settings_page};

mod pages;

#[derive(IntoElement)]
pub struct SettingsArea {
    id: ElementId,
    managers: Managers,
    settings_page: Entity<SharedString>,
}

impl SettingsArea {
    pub fn new(
        id: impl Into<ElementId>,
        managers: Managers,
        settings_page: Entity<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            managers,
            settings_page,
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
            .child(render_settings_page(cx, self.id, self.managers, self.settings_page))
    }
}
