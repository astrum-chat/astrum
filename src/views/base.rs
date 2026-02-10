use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, RenderOnce, StyleRefinement, Styled, Window,
    div, prelude::*, px,
};
use gpui_squircle::{SquircleStyled, squircle};
use gpui_tesserae::theme::ThemeExt;

#[cfg(target_os = "macos")]
use crate::views::MACOS_TITLEBAR_PADDING;
use crate::{blocks::TitleBar, views::FULLSCREEN_PADDING};

#[derive(IntoElement)]
pub struct BaseView {
    docked_left: Option<AnyElement>,
    docked_right: Option<AnyElement>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl BaseView {
    pub fn new() -> Self {
        Self {
            docked_left: None,
            docked_right: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    pub fn docked_left(mut self, elem: impl IntoElement) -> Self {
        self.docked_left = Some(elem.into_any_element());
        self
    }

    pub fn docked_right(mut self, elem: impl IntoElement) -> Self {
        self.docked_right = Some(elem.into_any_element());
        self
    }

    pub fn child(mut self, elem: impl IntoElement) -> Self {
        self.children.push(elem.into_any_element());
        self
    }
}

impl Styled for BaseView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for BaseView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        gpui_tesserae::init_for_window(window, cx);

        let base = div()
            .id("base_view")
            .tab_group()
            .tab_stop(false)
            .tab_index(0)
            .text_size(cx.get_theme().layout.text.default_font.sizes.body)
            .size_full()
            .bg(cx.get_theme().variants.active(cx).colors.background.primary)
            .flex()
            .pr(px(10.));

        #[cfg(target_os = "macos")]
        let base = base.when_else(
            window.is_fullscreen(),
            |this| this.pt(FULLSCREEN_PADDING),
            |this| this.pt(MACOS_TITLEBAR_PADDING),
        );

        #[cfg(not(target_os = "macos"))]
        let base = base.pt(FULLSCREEN_PADDING);

        let secondary_bg_color = cx
            .get_theme()
            .variants
            .active(cx)
            .colors
            .background
            .secondary;

        let content = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .justify_center()
            .child(
                squircle()
                    .absolute_expand()
                    .rounded_tl(px(8.))
                    .rounded_tr(px(8.))
                    .bg(secondary_bg_color),
            )
            .children(self.children);

        base.absolute()
            .map(|mut this| {
                this.style().refine(&self.style);
                this
            })
            .child(TitleBar::new())
            .children(self.docked_left)
            .child(content)
            .children(self.docked_right)
    }
}
