use std::sync::Arc;

use gpui::{ElementId, Window, div, prelude::*, px};
use gpui_tesserae::{ElementIdExt, theme::ThemeExt};
use smol::lock::RwLock;

use crate::{blocks::TitleBar, managers::Managers};

mod blocks;
use blocks::{ChatArea, Sidebar};

pub struct ChatView {
    id: ElementId,
    pub managers: Arc<RwLock<Managers>>,
}

impl ChatView {
    pub fn new(id: impl Into<ElementId>, managers: Arc<RwLock<Managers>>) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui_tesserae::init_for_window(window, cx);

        div()
            .text_size(cx.get_theme().layout.text.default_font.sizes.body)
            .size_full()
            .max_w_full()
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
            .child(ChatArea::new(
                self.id.with_suffix("chat_area"),
                self.managers.clone(),
            ))
    }
}
