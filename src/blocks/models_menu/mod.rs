mod models_cache;
pub use models_cache::{CachedModel, ModelsCache};

mod model_fetching;
pub use model_fetching::{
    ProviderConfigChange, fetch_all_models, fetch_all_models_with_source,
    observe_providers_for_refresh, prefetch_all_models, refetch_provider_models,
};

use std::sync::Arc;

use gpui::{
    App, ElementId, Entity, Hsla, IntoElement, SharedString, Window, div, prelude::*, px,
};
use gpui_tesserae::{
    ElementIdExt,
    components::Icon,
    components::select::{SelectItem, SelectItemsMap, SelectState},
};
use anyml::models::{Model, ModelParams, ModelQuant};

use crate::managers::Managers;
use schema::UniqueId;

#[derive(Clone)]
pub struct ModelSelection {
    pub provider_id: UniqueId,
    pub provider_name: String,
    pub model_id: String,
    pub parameters: Option<String>,
    pub quantization: Option<String>,
}

#[derive(Clone)]
pub struct ModelSelectItem {
    display_name: SharedString,
    icon_path: SharedString,
    selection: ModelSelection,
}

impl ModelSelectItem {
    pub fn new(
        provider_name: &str,
        model_id: String,
        display_name: &str,
        provider_id: UniqueId,
        parameters: Option<String>,
        quantization: Option<String>,
        icon_path: SharedString,
    ) -> Self {
        Self {
            display_name: display_name.to_string().into(),
            icon_path,
            selection: ModelSelection {
                provider_id,
                provider_name: provider_name.to_string(),
                model_id,
                parameters,
                quantization,
            },
        }
    }
}

impl SelectItem for ModelSelectItem {
    type Value = ModelSelection;

    fn name(&self) -> SharedString {
        self.display_name.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.selection
    }

    fn display(&self, _window: &mut Window, _cx: &App, text_color: Hsla) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .child(
                Icon::new(self.icon_path.clone())
                    .size(px(14.))
                    .color(text_color)
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .min_w_0()
                    .w_full()
                    .text_ellipsis()
                    .text_color(text_color)
                    .child(self.name()),
            )
    }
}

pub type OnModelItemClickFn = Box<
    dyn Fn(
        bool,
        Arc<SelectState<ModelSelection, ModelSelectItem>>,
        SharedString,
        &mut Window,
        &mut App,
    ),
>;

pub struct InitialModelSelection {
    pub provider_id: UniqueId,
    pub provider_name: String,
    pub model_id: String,
    pub parameters: Option<String>,
    pub quantization: Option<String>,
    pub icon_path: SharedString,
}

/// Creates the models select state with an empty items list.
/// Items are populated lazily when the menu is opened.
/// If `custom_on_item_click` is provided, it will be used instead of the default callback.
/// If `initial_selection` is provided, a placeholder item will be added and selected.
pub fn create_models_select_state(
    id: ElementId,
    managers: Managers,
    custom_on_item_click: Option<OnModelItemClickFn>,
    initial_selection: Option<InitialModelSelection>,
    window: &mut Window,
    cx: &mut App,
) -> SelectState<ModelSelection, ModelSelectItem> {
    let state_id = id.with_suffix("models_select_state");

    let mut state = SelectState::<ModelSelection, ModelSelectItem>::from_window(
        state_id,
        window,
        cx,
        |_window, _cx| SelectItemsMap::new(),
    );

    // Add a placeholder item and select it if initial selection is provided
    if let Some(selection) = initial_selection {
        let display_name = Model {
            id: selection.model_id.clone(),
            parameters: selection
                .parameters
                .as_deref()
                .filter(|p| !p.is_empty())
                .map(|p| ModelParams::new(p)),
            quantization: selection
                .quantization
                .as_deref()
                .filter(|q| !q.is_empty())
                .map(|q| ModelQuant::new(q)),
            thinking: None,
        }
        .to_string();
        let item = ModelSelectItem::new(
            &selection.provider_name,
            selection.model_id.clone(),
            &display_name,
            selection.provider_id,
            selection.parameters,
            selection.quantization,
            selection.icon_path,
        );
        let item_name = item.name();
        state.push_item(cx, item);
        let _ = state.select_item(cx, item_name);
    }

    if let Some(custom_callback) = custom_on_item_click {
        // Use the custom callback
        state.on_item_click(move |checked, state, item_name, window, cx| {
            custom_callback(checked, state, item_name, window, cx);
        });
    } else {
        // Set up the default selection callback
        let managers_for_callback = managers.clone();
        state.on_item_click(move |checked, state, item_name, _window, cx| {
            if !checked {
                state.hide_menu(cx);
                return;
            }

            // Get the selected item's value - clone values to avoid borrow conflict
            let selection = {
                let items = state.items.read(cx);
                items
                    .get(&item_name)
                    .map(|entry| entry.item.value().clone())
            };

            if let Some(selection) = selection {
                // Update the select state's selected item
                let _ = state.select_item(cx, item_name);

                // Update ModelsManager
                managers_for_callback.models.update(cx, |models, cx| {
                    models.set_current_selection(
                        cx,
                        selection.provider_id,
                        selection.provider_name,
                        selection.model_id,
                        selection.parameters,
                        selection.quantization,
                    );
                });
            }

            state.hide_menu(cx);
        });
    }

    state
}

/// Which model selection to use for auto-selecting in the picker
#[derive(Clone, Copy, Default, Debug)]
pub enum ModelSelectionSource {
    #[default]
    Current,
    ChatTitles,
}

/// Populates the select state from the cache.
/// Used to initialize the picker with cached models on creation.
pub fn populate_state_from_cache(
    state: &Arc<SelectState<ModelSelection, ModelSelectItem>>,
    models_cache: &Entity<ModelsCache>,
    current_provider_id: Option<&UniqueId>,
    current_model: Option<&String>,
    cx: &mut App,
) {
    let cached_models = models_cache.read(cx).get_all_models().to_vec();

    for cached in cached_models {
        let item = ModelSelectItem::new(
            &cached.provider_name,
            cached.model_id.clone(),
            &cached.display_name,
            cached.provider_id.clone(),
            cached.parameters.clone(),
            cached.quantization.clone(),
            cached.icon_path.clone(),
        );

        let item_name = item.name();
        state.push_item(cx, item);

        let provider_matches = current_provider_id == Some(&cached.provider_id);
        let model_matches = current_model == Some(&cached.model_id);

        if provider_matches && model_matches {
            let _ = state.select_item(cx, item_name);
        }
    }
}
