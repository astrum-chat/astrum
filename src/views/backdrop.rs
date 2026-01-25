use gpui::{
    AnyElement, App, DefiniteLength, Div, Edges, ElementId, Hsla, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};
use gpui_tesserae::theme::ThemeExt;
use gpui_transitions::{BoolLerp, Transition};
use smallvec::{SmallVec, smallvec};

#[derive(Default)]
pub struct BackdropViewStyles {
    pub bg: Option<Hsla>,
    pub opacity: Option<f32>,
    pub padding: Edges<Option<DefiniteLength>>,
}

#[derive(IntoElement)]
pub struct BackdropView {
    id: ElementId,
    style: BackdropViewStyles,
    transition: Transition<BoolLerp<f32>>,
    children: SmallVec<[AnyElement; 2]>,
}

impl BackdropView {
    pub fn new(id: impl Into<ElementId>, transition: Transition<BoolLerp<f32>>) -> Self {
        Self {
            id: id.into(),
            style: BackdropViewStyles {
                bg: None,
                opacity: None,
                padding: Edges::all(Some(px(85.).into())),
            },
            transition,
            children: smallvec![],
        }
    }

    pub fn bg(mut self, bg: impl Into<Hsla>) -> Self {
        self.style.bg = Some(bg.into());
        self
    }

    /// Sets the opacity of the backdrop background (0.0 to 1.0). Default is 0.78.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = Some(opacity);
        self
    }

    /// Sets uniform outer padding for all sides.
    pub fn p(mut self, padding: impl Into<DefiniteLength>) -> Self {
        let padding = padding.into();
        self.style.padding = Edges::all(Some(padding));
        self
    }

    /// Sets top outer padding.
    pub fn pt(mut self, padding: impl Into<DefiniteLength>) -> Self {
        self.style.padding.top = Some(padding.into());
        self
    }

    /// Sets bottom outer padding.
    pub fn pb(mut self, padding: impl Into<DefiniteLength>) -> Self {
        self.style.padding.bottom = Some(padding.into());
        self
    }

    /// Sets left outer padding.
    pub fn pl(mut self, padding: impl Into<DefiniteLength>) -> Self {
        self.style.padding.left = Some(padding.into());
        self
    }

    /// Sets right outer padding.
    pub fn pr(mut self, padding: impl Into<DefiniteLength>) -> Self {
        self.style.padding.right = Some(padding.into());
        self
    }
}

impl ParentElement for BackdropView {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

const MARGIN: f32 = 0.1;

impl RenderOnce for BackdropView {
    fn render(self, window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let color = self.style.bg.unwrap_or_else(|| {
            cx.get_theme()
                .variants
                .active(cx)
                .colors
                .background
                .primary
                .into()
        });

        let padding = self.style.padding;
        let backdrop_opacity = &self.transition.evaluate(window, cx).value();
        let bg_opacity = self.style.opacity.unwrap_or(0.85);

        div()
            .id(self.id)
            .tab_group()
            .tab_index(0)
            .absolute()
            .inset_0()
            .bg(color.opacity(bg_opacity))
            .opacity(*backdrop_opacity)
            .when_some(padding.top, |this, pt| this.pt(pt))
            .when_some(padding.bottom, |this, pb| this.pb(pb))
            .when_some(padding.left, |this, pl| this.pl(pl))
            .when_some(padding.right, |this, pr| this.pr(pr))
            .child(backdrop_children_wrapper(self.children))
            .cursor(gpui::CursorStyle::default())
            .map(|this| {
                let transition = self.transition.clone();

                this.on_click(move |_event, _window, cx| {
                    transition.update(cx, |this, cx| {
                        *this = false.into();
                        cx.notify();
                    });
                })
            })
            .map(|this| {
                let backdrop_opacity_1 = backdrop_opacity.clone();
                let backdrop_opacity_2 = backdrop_opacity.clone();
                let backdrop_opacity_3 = backdrop_opacity.clone();

                this.on_mouse_move(move |_event, _window, cx| {
                    if backdrop_opacity_1 <= MARGIN {
                        return;
                    };

                    cx.stop_propagation();
                })
                .on_any_mouse_down(move |_event, _window, cx| {
                    if backdrop_opacity_2 <= MARGIN {
                        return;
                    };

                    cx.stop_propagation();
                })
                .map(|mut this| {
                    this.interactivity()
                        .on_any_mouse_up(move |_event, _window, cx| {
                            if backdrop_opacity_3 <= MARGIN {
                                return;
                            };

                            cx.stop_propagation();
                        });

                    this
                })
            })
    }
}

fn backdrop_children_wrapper(children: SmallVec<[AnyElement; 2]>) -> Div {
    div()
        .h_full()
        .w_full()
        .on_mouse_move(move |_event, _window, cx| {
            cx.stop_propagation();
        })
        .on_any_mouse_down(move |_event, _window, cx| {
            cx.stop_propagation();
        })
        .map(|mut this| {
            this.interactivity()
                .on_any_mouse_up(move |_event, _window, cx| {
                    cx.stop_propagation();
                });

            this
        })
        .children(children)
}
