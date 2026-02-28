use gpui_tesserae::{ElementIdExt, theme::ThemeExt};
use phf::phf_map;

use gpui::{
    AnyElement, App, ElementId, Entity, IntoElement, ParentElement, SharedString, Styled, div, px,
    relative,
};

mod providers_page;
pub use providers_page::*;

mod chat_titles_page;
pub use chat_titles_page::*;

mod system_prompt_page;
pub use system_prompt_page::*;

use crate::managers::Managers;

const SETTING_PAGES: phf::Map<&str, fn(ElementId, Managers) -> AnyElement> = phf_map! {
    "Providers" => |id, managers| {
        ProvidersPage::new(id, managers).into_any_element()
    },
    "Chat Titles" => |id, managers| {
        ChatTitlesPage::new(id, managers).into_any_element()
    },
    "System Prompt" => |id, managers| {
        SystemPromptPage::new(id, managers).into_any_element()
    }
};

const INVALID_SETTING_PAGE: fn(ElementId, Managers) -> AnyElement =
    |_id, _managers| div().into_any_element();

pub fn render_settings_page(
    cx: &mut App,
    base_id: impl Into<ElementId>,
    managers: Managers,
    settings_page: Entity<SharedString>,
) -> impl IntoElement {
    let current_settings_page_name = settings_page.read(cx).clone();

    let render = SETTING_PAGES
        .get(current_settings_page_name.as_str())
        .unwrap_or(&INVALID_SETTING_PAGE);

    div().w_full().max_w(px(650.)).h_full().child(render(
        base_id
            .into()
            .with_suffix("page")
            .with_suffix(current_settings_page_name),
        managers,
    ))
}

pub fn render_settings_page_title(
    cx: &App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
) -> impl IntoElement {
    let heading_md_text_size = cx
        .get_theme()
        .layout
        .text
        .default_font
        .sizes
        .heading_md
        .clone();
    let primary_text_color = cx.get_theme().variants.active(cx).colors.text.primary;

    let body_text_size = cx.get_theme().layout.text.default_font.sizes.body.clone();
    let secondary_text_color = cx.get_theme().variants.active(cx).colors.text.secondary;

    div()
        .w_full()
        .min_w_0()
        .h_auto()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .w_full()
                .min_w_0()
                .h_auto()
                .text_size(heading_md_text_size)
                .text_color(primary_text_color)
                .line_height(relative(1.))
                .child(title.into()),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .h_auto()
                .text_size(body_text_size)
                .text_color(secondary_text_color)
                .child(description.into()),
        )
}
