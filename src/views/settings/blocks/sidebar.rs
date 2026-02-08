use std::sync::Arc;

use gpui::{
    App, ElementId, Entity, InteractiveElement, IntoElement, RenderOnce, SharedString, div,
    prelude::*, px,
};
use gpui_tesserae::{
    ElementIdExt,
    components::{Toggle, ToggleVariant},
    extensions::mouse_handleable::MouseHandleable,
    theme::ThemeExt,
};
use smol::lock::RwLock;

use crate::{assets::AstrumIconKind, managers::Managers};

const SETTING_PAGES: &[(AstrumIconKind, &str)] = &[
    (AstrumIconKind::Key, "Providers"),
    (AstrumIconKind::Title, "Chat Titles"),
];

#[derive(IntoElement)]
pub struct Sidebar {
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
}

impl Sidebar {
    pub fn new(id: impl Into<ElementId>, managers: Arc<RwLock<Managers>>) -> Self {
        let id = id.into();

        Self {
            id: id.clone(),
            managers,
        }
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let current_settings_page_name_state = &self
            .managers
            .read_arc_blocking()
            .settings
            .current_settings_page_name;

        let current_settings_page_name = current_settings_page_name_state.read(cx);

        let top_section = div()
            .pl(px(10.))
            .pr(px(10.))
            .flex()
            .flex_col()
            .gap(px(5.))
            .children(SETTING_PAGES.iter().map(|(icon, name)| {
                render_settings_page_toggle(
                    &self.id,
                    (icon, name),
                    current_settings_page_name_state.clone(),
                    current_settings_page_name,
                )
            }));

        let secondary_text_color = cx.get_theme().variants.active(cx).colors.text.secondary;
        let caption_size = cx.get_theme().layout.text.default_font.sizes.caption;

        let version_label = div().pl(px(14.)).pb(px(10.)).child(
            div()
                .text_size(caption_size)
                .text_color(secondary_text_color)
                .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
        );

        div()
            .id(self.id)
            .tab_group()
            .tab_index(0)
            .tab_stop(false)
            .min_w(px(300.))
            .h_full()
            .flex()
            .flex_col()
            .justify_between()
            .child(top_section)
            .child(version_label)
    }
}

fn render_settings_page_toggle(
    base_id: &ElementId,
    (icon, name): (&AstrumIconKind, &'static str),
    current_settings_page_name_state: Entity<SharedString>,
    current_settings_page_name: &SharedString,
) -> impl IntoElement {
    Toggle::new(base_id.with_suffix(name))
        .text(name)
        .icon(icon)
        .icon_size(px(14.))
        .variant(ToggleVariant::Secondary)
        .checked(current_settings_page_name == name)
        .on_click(move |_checked, _window, cx| {
            current_settings_page_name_state
                .update(cx, |this, _cx| *this = SharedString::new(name));
        })
        .justify_start()
}
