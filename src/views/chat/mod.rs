use std::sync::Arc;

use gpui::{ElementId, Window, prelude::*};
use gpui_tesserae::ElementIdExt;
use smol::lock::RwLock;

use crate::{managers::Managers, views::BaseView};

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
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        BaseView::new()
            .docked_left(Sidebar::new(
                self.id.with_suffix("sidebar"),
                self.managers.clone(),
            ))
            .child(ChatArea::new(
                self.id.with_suffix("chat_area"),
                self.managers.clone(),
            ))
    }
}
