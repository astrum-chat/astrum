use gpui::{AnyWindowHandle, ElementId, PromptLevel, Window, prelude::*};
use gpui_tesserae::ElementIdExt;

use crate::{managers::Managers, views::BaseView};

mod blocks;
use blocks::{ChatArea, Sidebar};

pub struct ChatView {
    id: ElementId,
    pub managers: Managers,
}

impl ChatView {
    pub fn new(id: impl Into<ElementId>, managers: Managers) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }

    pub fn observe_errors(&self, window_handle: AnyWindowHandle, cx: &mut Context<Self>) {
        let errors = self.managers.errors.clone();
        cx.observe(&errors, move |_this, errors, cx| {
            let message = errors.update(cx, |errors, _cx| errors.pop_front());
            if let Some(message) = message {
                let _ = window_handle.update(cx, |_, window, cx| {
                    let _ = window.prompt(
                        PromptLevel::Critical,
                        "Error",
                        Some(&message),
                        &["OK"],
                        cx,
                    );
                });
            }
        })
        .detach();
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
