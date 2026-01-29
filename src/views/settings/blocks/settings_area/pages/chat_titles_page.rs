use std::sync::Arc;

use gpui::{App, ElementId, Overflow, PointRefinement, Window, div, prelude::*, px, relative};
use gpui_squircle::{SquircleStyled, squircle};
use gpui_tesserae::{
    ElementIdExt,
    components::select::Select,
    primitives::min_w0_wrapper,
    theme::{ThemeExt, ThemeLayerKind},
};
use smol::lock::RwLock;

use crate::{
    blocks::ModelPicker, managers::Managers,
    views::settings::blocks::settings_area::pages::render_settings_page_title,
};

#[derive(IntoElement)]
pub struct ChatTitlesPage {
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
}

impl ChatTitlesPage {
    pub fn new(id: impl Into<ElementId>, managers: Arc<RwLock<Managers>>) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl RenderOnce for ChatTitlesPage {
    fn render(self, window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(render_settings_page_title(
                cx,
                "Chat Titles",
                "Customize how chat titles are generated.",
            ))
            .child(
                div()
                    .id(self.id.clone())
                    .w_full()
                    .h_full()
                    .flex()
                    .flex_col()
                    .pb(px(20.))
                    .gap(px(10.))
                    .map(|mut this| {
                        this.style().overflow = PointRefinement {
                            x: None,
                            y: Some(Overflow::Scroll),
                        };
                        this
                    })
                    .child(render_model_picker(
                        self.id.with_suffix("model_picker"),
                        self.managers.clone(),
                        window,
                        cx,
                    )),
            )
    }
}

fn render_model_picker(
    id: impl Into<ElementId>,
    managers: Arc<RwLock<Managers>>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id = id.into();

    let layer_kind = ThemeLayerKind::Tertiary;
    let background_color = layer_kind.resolve(cx);
    let border_color = layer_kind.next().resolve(cx);
    let primary_text_color = cx.get_theme().variants.active(cx).colors.text.primary;
    let secondary_text_color = cx.get_theme().variants.active(cx).colors.text.secondary;
    let text_heading_sm_size = cx.get_theme().layout.text.default_font.sizes.heading_sm;
    let text_body_size = cx.get_theme().layout.text.default_font.sizes.body;
    let corner_radius = cx.get_theme().layout.corner_radii.lg;
    let padding = cx.get_theme().layout.padding.xl;

    // Create model picker with custom on_item_click for chat titles page
    let picker = ModelPicker::new(
        id.clone(),
        managers,
        true,
        Some(Box::new(|checked, state, item_name, _window, cx| {
            println!("hello world");
            if checked {
                let _ = state.select_item(cx, item_name);
            }
            state.hide_menu(cx);
        })),
        window,
        cx,
    );

    let top_content = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(padding / 2.)
        .child(
            min_w0_wrapper()
                .text_size(text_heading_sm_size)
                .text_color(primary_text_color)
                .line_height(relative(1.))
                .child("Model"),
        )
        .child(
            min_w0_wrapper()
                .text_size(text_body_size)
                .text_color(secondary_text_color)
                .child("Set the model used to generate titles. Small local models are preferable."),
        );

    let bottom_content = Select::new(id.with_suffix("model_select"), picker.state)
        .max_w_full()
        .max_menu_h(px(200.))
        .layer(ThemeLayerKind::Quaternary)
        .disabled(picker.has_no_providers);

    div()
        .w_full()
        .h_auto()
        .flex()
        .flex_col()
        .p(padding)
        .gap(padding)
        .child(
            squircle()
                .absolute_expand()
                .bg(background_color)
                .border(px(1.))
                .border_color(border_color)
                .border_inside()
                .rounded(corner_radius),
        )
        .child(top_content)
        .child(bottom_content)
}
