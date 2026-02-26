use std::sync::Arc;
use std::time::Duration;

use gpui::{App, ElementId, Entity, IntoElement, RenderOnce, div, ease_out_quint, prelude::*, px};
use gpui_tesserae::{
    ElementIdExt, PositionalParentElement, conitional_transition,
    components::{Button, ButtonVariant, Toggle, ToggleVariant},
    extensions::mouse_handleable::MouseHandleable,
    theme::ThemeExt,
};
use smol::lock::RwLock;

use schema::UniqueId;

use crate::{assets::AstrumIconKind, managers::Managers};

#[derive(IntoElement)]
pub struct ThreadToggle {
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
    chat_id: UniqueId,
    title: String,
    checked: bool,
    current_chat_id_state: Entity<Option<UniqueId>>,
}

impl ThreadToggle {
    pub fn new(
        id: impl Into<ElementId>,
        managers: Arc<RwLock<Managers>>,
        chat_id: UniqueId,
        title: String,
        checked: bool,
        current_chat_id_state: Entity<Option<UniqueId>>,
    ) -> Self {
        Self {
            id: id.into(),
            managers,
            chat_id,
            title,
            checked,
            current_chat_id_state,
        }
    }
}

impl RenderOnce for ThreadToggle {
    fn render(self, window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let md_corner_radii = cx.get_theme().layout.corner_radii.md;
        let chat_id_for_select = self.chat_id.clone();
        let current_chat_id_state = self.current_chat_id_state.clone();

        let is_hovered =
            window.use_keyed_state(self.id.with_suffix("state:hover"), cx, |_window, _cx| false);
        let hovered = *is_hovered.read(cx);

        let is_hovered_for_callback = is_hovered.clone();

        let opacity_transition = conitional_transition!(
            self.id.with_suffix("state:transition:delete_opacity"),
            window,
            cx,
            Duration::from_millis(250),
            {
                hovered => 1.0_f32,
                _ => 0.0_f32
            }
        )
        .with_easing(ease_out_quint());
        let delete_opacity = *opacity_transition.evaluate(window, cx);

        let delete_chat_id = self.chat_id.clone();
        let managers = self.managers.clone();
        let delete_button = div()
            .h_0()
            .flex()
            .items_center()
            .justify_center()
            .opacity(delete_opacity)
            .child(
                Button::new(self.id.with_suffix("delete_btn"))
                    .variant(ButtonVariant::DestructiveGhost)
                    .icon(AstrumIconKind::Trash)
                    .p(px(6.))
                    // 5 is the amount of padding around the button.
                    .rounded(md_corner_radii - px((5f32 / 2.).floor()))
                    .on_click(move |_event, _window, cx| {
                        managers
                            .write_arc_blocking()
                            .chats
                            .delete_chat(cx, delete_chat_id.clone());
                    }),
            );

        div().w_full().pb(px(5.)).child(
            Toggle::new(self.id)
                .text(self.title)
                .variant(ToggleVariant::Secondary)
                .checked(self.checked)
                .icon(AstrumIconKind::Chat)
                .on_click(move |_checked, _window, cx| {
                    current_chat_id_state
                        .update(cx, |this, _cx| *this = Some(chat_id_for_select.clone()));
                })
                .on_hover(move |is_hover, _window, cx| {
                    is_hovered_for_callback.update(cx, |state, cx| {
                        *state = *is_hover;
                        cx.notify();
                    });
                })
                .justify_between()
                .pr(px(5.))
                .gap(px(5.))
                .child_right(delete_button),
        )
    }
}
