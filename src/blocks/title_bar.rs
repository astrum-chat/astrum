use gpui::{
    InteractiveElement, IntoElement, MouseButton, RenderOnce, Styled, div, prelude::FluentBuilder,
    px,
};

#[derive(IntoElement)]
pub struct TitleBar {}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut gpui::Window, _cx: &mut gpui::App) -> impl IntoElement {
        div()
            .w_full()
            .h(px(32.))
            .top(px(0.))
            .absolute()
            .when(window.is_fullscreen(), |this| this.invisible())
            .on_mouse_down(MouseButton::Left, |event, window, _| {
                if event.click_count != 2 {
                    return;
                }
                window.zoom_window();
            })
    }
}

impl TitleBar {
    pub fn new() -> Self {
        Self {}
    }
}
