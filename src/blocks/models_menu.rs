use std::sync::Arc;

use gpui::{
    App, AsyncApp, ElementId, Entity, Hsla, IntoElement, SharedString, Window, div, prelude::*,
};
use gpui_tesserae::{
    ElementIdExt,
    components::select::{SelectItem, SelectItemsMap, SelectState},
};
use smol::lock::RwLock;

use crate::{Managers, managers::Provider, managers::UniqueId, utils::FrontInsertMap};

/// Value type for ModelSelectItem containing both provider and model info.
#[derive(Clone)]
pub struct ModelSelection {
    pub provider_id: UniqueId,
    pub provider_name: String,
    pub model_id: String,
}

/// Represents a model item in the select menu.
/// Contains both display information and selection data.
#[derive(Clone)]
pub struct ModelSelectItem {
    /// Display name shown in the menu (e.g., "Ollama: deepseek-r1:14b")
    display_name: SharedString,
    /// The selection value containing provider and model IDs
    selection: ModelSelection,
}

impl ModelSelectItem {
    pub fn new(provider_name: &str, model_id: String, provider_id: UniqueId) -> Self {
        Self {
            display_name: format!("{}/{}", provider_name.to_lowercase(), model_id).into(),
            selection: ModelSelection {
                provider_id,
                provider_name: provider_name.to_string(),
                model_id,
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
            .text_ellipsis()
            .text_color(text_color)
            .child(self.name())
    }
}

/// Callback type for custom on_item_click handlers.
pub type OnModelItemClickFn = Box<
    dyn Fn(
        bool,
        Arc<SelectState<ModelSelection, ModelSelectItem>>,
        SharedString,
        &mut Window,
        &mut App,
    ),
>;

/// Initial selection data for pre-populating the select state
pub struct InitialModelSelection {
    pub provider_id: UniqueId,
    pub provider_name: String,
    pub model_id: String,
}

/// Creates the models select state with an empty items list.
/// Items are populated lazily when the menu is opened.
/// If `custom_on_item_click` is provided, it will be used instead of the default callback.
/// If `initial_selection` is provided, a placeholder item will be added and selected.
pub fn create_models_select_state(
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
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
        let item = ModelSelectItem::new(
            &selection.provider_name,
            selection.model_id,
            selection.provider_id,
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
                let mut managers = managers_for_callback.write_arc_blocking();
                managers.models.set_current_provider(
                    cx,
                    selection.provider_id,
                    selection.provider_name,
                );
                managers.models.set_current_model(cx, selection.model_id);
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

/// Fetches models from all providers asynchronously and populates the select state.
pub fn fetch_all_models(
    managers: Arc<RwLock<Managers>>,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    cx: &mut App,
) {
    fetch_all_models_with_source(managers, state, ModelSelectionSource::Current, cx);
}

/// Fetches models from all providers asynchronously and populates the select state.
/// Uses the specified source to determine which model to auto-select.
pub fn fetch_all_models_with_source(
    managers: Arc<RwLock<Managers>>,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    source: ModelSelectionSource,
    cx: &mut App,
) {
    // Get current provider_id and model before spawning async task
    let (current_provider_id, current_model): (Option<UniqueId>, Option<String>) = {
        let managers = managers.read_arc_blocking();
        match source {
            ModelSelectionSource::Current => (
                managers.models.current_model.provider_id.read(cx).clone(),
                managers.models.get_current_model(cx).cloned(),
            ),
            ModelSelectionSource::ChatTitles => (
                managers
                    .models
                    .chat_titles_model
                    .provider_id
                    .read(cx)
                    .clone(),
                managers.models.get_chat_titles_model(cx).cloned(),
            ),
        }
    };

    cx.spawn(async move |cx: &mut AsyncApp| {
        // Collect provider info we need for fetching
        let providers_info: Vec<(UniqueId, crate::managers::Provider)> = {
            let managers = managers.read_arc_blocking();

            cx.read_entity(&managers.models.providers, |providers, _cx| {
                providers
                    .iter()
                    .map(|(id, p)| (id.clone(), p.as_ref().clone()))
                    .collect::<Vec<(UniqueId, crate::managers::Provider)>>()
            })
        };

        for (provider_id, provider) in providers_info {
            let provider_name: SharedString =
                cx.read_entity(&provider.name, |name: &SharedString, _| name.clone());

            match provider.inner.list_models().await {
                Ok(models) => {
                    let _ = cx.update(|cx| {
                        for model in models {
                            let item = ModelSelectItem::new(
                                &provider_name,
                                model.id.clone(),
                                provider_id.clone(),
                            );

                            // Select this item if it matches the current provider and model
                            let item_name = item.name();
                            state.push_item(cx, item);

                            let provider_matches =
                                current_provider_id.as_ref() == Some(&provider_id);
                            let model_matches = current_model.as_ref() == Some(&model.id);

                            if provider_matches && model_matches {
                                let _ = state.select_item(cx, item_name);
                            }
                        }
                    });
                }
                Err(err) => {
                    eprintln!("Failed to fetch models from {}: {}", provider_name, err);
                }
            }
        }
    })
    .detach();
}

/// Observes the providers entity and clears the models menu when providers change.
/// Also deselects the current model if its provider no longer exists.
pub fn observe_providers_for_refresh(
    providers: &Entity<FrontInsertMap<UniqueId, Arc<Provider>>>,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    managers: Arc<RwLock<Managers>>,
    cx: &mut App,
) {
    cx.observe(providers, move |providers, cx| {
        // Clear the menu items
        state.items.update(cx, |items, cx| {
            *items = SelectItemsMap::new();
            cx.notify();
        });

        // Clear the SelectState selection
        state.remove_selection(cx);

        // Check if current provider still exists, if not clear the manager selection
        let mut managers = managers.write_arc_blocking();
        let current_provider_id = managers.models.current_model.provider_id.read(cx).clone();

        if let Some(provider_id) = current_provider_id {
            let provider_exists = providers.read(cx).get(&provider_id).is_some();
            if !provider_exists {
                managers.models.clear_current_selection(cx);
            }
        }

        // Check if chat_titles provider still exists, if not clear the manager selection
        let chat_titles_provider_id = managers
            .models
            .chat_titles_model
            .provider_id
            .read(cx)
            .clone();

        if let Some(provider_id) = chat_titles_provider_id {
            let provider_exists = providers.read(cx).get(&provider_id).is_some();
            if !provider_exists {
                managers.models.clear_chat_titles_selection(cx);
            }
        }
    })
    .detach();
}
