use gpui::{App, ElementId, InteractiveElement, IntoElement, RenderOnce, div, prelude::*, px};
use gpui_squircle::{SquircleStyled, squircle};
use gpui_tesserae::{
    ElementIdExt, PositionalParentElement,
    components::{Icon, Input},
    primitives::input::InputState,
    theme::ThemeExt,
};

use crate::assets::AstrumIconKind;

#[derive(IntoElement)]
pub struct ModelDiscoveryView {
    id: ElementId,
}

impl ModelDiscoveryView {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into() }
    }
}

impl RenderOnce for ModelDiscoveryView {
    fn render(self, window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let secondary_bg_color = cx
            .get_theme()
            .variants
            .active(cx)
            .colors
            .background
            .secondary;

        let tertiary_bg_color = cx
            .get_theme()
            .variants
            .active(cx)
            .colors
            .background
            .tertiary;

        let search_input_state = window.use_keyed_state(
            self.id.with_suffix("state:search_models_input"),
            cx,
            |_window, cx| InputState::new(cx),
        );

        let search_bar = Input::new(
            self.id.with_suffix("search_models_input"),
            search_input_state,
        )
        .placeholder("Search Models")
        .child_left(Icon::new(AstrumIconKind::Search));

        div()
            .id(self.id)
            .tab_group()
            .tab_index(0)
            .tab_stop(false)
            .size_full()
            .flex()
            .flex_col()
            .p(px(20.))
            .child(
                squircle()
                    .absolute_expand()
                    .rounded(px(8.))
                    .bg(secondary_bg_color)
                    .border(px(1.))
                    .border_color(tertiary_bg_color)
                    .border_inside(),
            )
            .child(search_bar)
    }
}
