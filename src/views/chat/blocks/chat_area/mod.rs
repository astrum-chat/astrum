use gpui::{App, ElementId, IntoElement, RenderOnce, div, prelude::*, px};

use notitia::prelude::*;
use notitia_gpui::WindowNotitiaExt;

use schema::{AstrumDb, MessageRecord};

use crate::managers::Managers;

mod chat_actions;
mod chat_box;
mod existing_chat;
mod md_render;
mod prompt_new_chat;

use chat_box::ChatBox;
use existing_chat::render_existing_chat;
use prompt_new_chat::render_prompt_new_chat;

#[derive(IntoElement)]
pub struct ChatArea {
    id: ElementId,
    managers: Managers,
}

impl ChatArea {
    pub fn new(id: impl Into<ElementId>, managers: Managers) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl RenderOnce for ChatArea {
    fn render(self, window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let (current_chat_id, db_initialized, db) =
            self.managers.chats.read_with(cx, |chats, cx| {
                (
                    chats.get_current_chat_id().read(cx).clone(),
                    chats.db_initialized(),
                    if chats.db_initialized() {
                        Some(chats.db().clone())
                    } else {
                        None
                    },
                )
            });

        let messages = current_chat_id
            .as_ref()
            .filter(|_| db_initialized)
            .and_then(|chat_id| {
                let db = db.as_ref()?;
                let chat_id_for_query = chat_id.clone();
                Some(window.use_keyed_db_query(
                    format!("messages_{}", chat_id),
                    cx,
                    |_window, _cx| {
                        db.query(
                            AstrumDb::MESSAGES
                                .select((
                                    MessageRecord::ID,
                                    MessageRecord::ROLE,
                                    MessageRecord::CONTENT,
                                ))
                                .filter(MessageRecord::CHAT_ID.eq(chat_id_for_query.clone()))
                                .order_by(MessageRecord::CREATED_AT, OrderDirection::Asc)
                                .fetch_all::<BTreeMap<_, _>>(),
                        )
                    },
                ))
            });

        div()
            .id(self.id.clone())
            .h_full()
            .w_full()
            .max_w(px(800.))
            .flex()
            .flex_col()
            .items_start()
            .justify_between()
            .map(|this| {
                match &messages {
                    Some(messages) => match messages.read(cx) {
                        Some(msgs) => this.child(render_existing_chat(&self.id, msgs)),
                        None => this.child(div()), // spacer so justify_between keeps chat box at bottom
                    },
                    None => this.child(render_prompt_new_chat(window, cx)),
                }
            })
            .child(
                div()
                    .w_full()
                    .p(px(20.))
                    .pt(px(0.))
                    .child(ChatBox::new(
                        self.id.clone(),
                        self.managers.clone(),
                    )),
            )
    }
}
