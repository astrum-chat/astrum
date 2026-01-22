use std::sync::Arc;

use gpui::{App, ElementId, InteractiveElement, IntoElement, RenderOnce, div, prelude::*, px};
use gpui_squircle::{SquircleStyled, squircle};
use gpui_tesserae::theme::ThemeExt;
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
        let secondary_bg_color = cx
            .get_theme()
            .variants
            .active(cx)
            .colors
            .background
            .secondary;

        div()
            .id(self.id.clone())
            .tab_group()
            .tab_index(1)
            .tab_stop(false)
            .size_full()
            .flex()
            .justify_center()
            .p(px(20.))
            .pb(px(0.))
            .child(
                squircle()
                    .absolute_expand()
                    .rounded_tl(px(8.))
                    .rounded_tr(px(8.))
                    .bg(secondary_bg_color),
            )
            .child(render_settings_page(cx, self.id, self.managers))
    }
}
