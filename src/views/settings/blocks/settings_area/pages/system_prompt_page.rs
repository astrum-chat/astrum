use gpui::{
    App, ElementId, Entity, Focusable, Overflow, PointRefinement, Window, div, prelude::*, px,
    relative,
};
use gpui_squircle::{SquircleStyled, squircle};
use gpui_tesserae::{
    ElementIdExt,
    components::Input,
    primitives::{input::InputState, min_w0_wrapper},
    theme::{ThemeExt, ThemeLayerKind},
};
use schema::{AstrumDb, SystemPromptRecord, UniqueId};
use tracing::error;

use crate::{
    managers::Managers, views::settings::blocks::settings_area::pages::render_settings_page_title,
};

#[derive(IntoElement)]
pub struct SystemPromptPage {
    id: ElementId,
    managers: Managers,
}

impl SystemPromptPage {
    pub fn new(id: impl Into<ElementId>, managers: Managers) -> Self {
        Self {
            id: id.into(),
            managers,
        }
    }
}

impl RenderOnce for SystemPromptPage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(render_settings_page_title(
                cx,
                "System Prompt",
                "Set custom instructions that apply to every conversation.",
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
                    .child(render_prompt_input(
                        self.id.with_suffix("prompt_input"),
                        self.managers,
                        window,
                        cx,
                    )),
            )
    }
}

fn save_system_prompt(managers: &Managers, input_state: &Entity<InputState>, cx: &mut App) {
    let new_value = input_state.read(cx).value().to_string();
    let current = managers.system_prompt.read(cx).to_string();
    if new_value == current {
        return;
    }

    managers
        .system_prompt
        .update(cx, |s, _cx| *s = new_value.clone().into());

    let db = managers.chats.read(cx).db().clone();
    cx.spawn(async move |_cx| {
        if let Err(e) = db.mutate(AstrumDb::SYSTEM_PROMPTS.delete()).execute().await {
            error!("Failed to delete system prompt: {e}");
            return;
        }

        if !new_value.is_empty() {
            if let Err(e) = db
                .mutate(
                    AstrumDb::SYSTEM_PROMPTS.insert(
                        SystemPromptRecord::build()
                            .id(UniqueId::new())
                            .content(new_value),
                    ),
                )
                .execute()
                .await
            {
                error!("Failed to save system prompt: {e}");
            }
        }
    })
    .detach();
}

fn render_prompt_input(
    id: impl Into<ElementId>,
    managers: Managers,
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

    let system_prompt_value = managers.system_prompt.read(cx).clone();

    let input_state =
        window.use_keyed_state(id.with_suffix("state:prompt_input"), cx, |_window, cx| {
            InputState::new(cx).initial_value(&system_prompt_value)
        });

    let managers_for_blur = managers.clone();
    let input_state_for_blur = input_state.clone();
    let managers_for_close = managers.clone();
    let input_state_for_close = input_state.clone();

    let input = Input::new(id.with_suffix("input"), input_state.clone())
        .multiline()
        .multiline_max_lines(12)
        .multiline_min_lines(3)
        .multiline_wrapped()
        .placeholder("Enter your system prompt...")
        .layer(ThemeLayerKind::Quaternary);

    let _subs = window.use_keyed_state(id.with_suffix("state:subs"), cx, |window, cx| {
        let sub1 = window.on_focus_out(&input.focus_handle(cx), cx, move |_event, _window, cx| {
            save_system_prompt(&managers_for_blur, &input_state_for_blur, cx);
        });
        let sub2 = window.on_window_should_close(cx, move |_window, cx| {
            save_system_prompt(&managers_for_close, &input_state_for_close, cx);
            true
        });
        (sub1, sub2)
    });

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
                .child("Instructions"),
        )
        .child(
            min_w0_wrapper()
                .text_size(text_body_size)
                .text_color(secondary_text_color)
                .child("This prompt is prepended to every conversation as a system message."),
        );

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
        .child(input)
}
