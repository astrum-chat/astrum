use std::sync::Arc;

use gpui::{
    Context, ElementId, IntoElement, ParentElement, Render, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_tesserae::{ElementIdExt, theme::ThemeExt};
use smol::lock::RwLock;

use crate::{blocks::TitleBar, managers::Managers, views::settings::blocks::SettingsArea};

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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui_tesserae::init_for_window(window, cx);

        div()
            .text_size(cx.get_theme().layout.text.default_font.sizes.body)
            .size_full()
            .bg(cx.get_theme().variants.active(cx).colors.background.primary)
            .flex()
            .pr(px(10.))
            .pt(px(10.))
            .when_else(
                window.is_fullscreen(),
                |this| this.pt(px(10.)),
                |this| this.pt(px(33.)),
            )
            .absolute()
            .child(TitleBar::new())
            .child(Sidebar::new(
                self.id.with_suffix("sidebar"),
                self.managers.clone(),
            ))
            .child(SettingsArea::new(
                self.id.with_suffix("settings_area"),
                self.managers.clone(),
            ))
    }
}
