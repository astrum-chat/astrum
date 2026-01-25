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

/// Creates the models select state with an empty items list.
/// Items are populated lazily when the menu is opened.
pub fn create_models_select_state(
    id: ElementId,
    managers: Arc<RwLock<Managers>>,
    window: &mut Window,
    cx: &mut App,
) -> SelectState<ModelSelection, ModelSelectItem> {
    let mut state = SelectState::<ModelSelection, ModelSelectItem>::from_window(
        id.with_suffix("models_select_state"),
        window,
        cx,
        |_window, _cx| SelectItemsMap::new(),
    );

    // Set up the selection callback
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
            managers
                .models
                .set_current_provider(cx, selection.provider_id);
            managers.models.set_current_model(cx, selection.model_id);
        }

        state.hide_menu(cx);
    });

    state
}

/// Fetches models from all providers asynchronously and populates the select state.
pub fn fetch_all_models(
    managers: Arc<RwLock<Managers>>,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    cx: &mut App,
) {
    // Get current model before spawning async task
    let current_model: Option<String> = managers
        .read_arc_blocking()
        .models
        .get_current_model(cx)
        .cloned();

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

                            // Select this item if it matches the current model
                            let item_name = item.name();
                            state.push_item(cx, item);

                            if current_model.as_ref() == Some(&model.id) {
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
/// This ensures the menu is refreshed with updated provider/model data on next open.
pub fn observe_providers_for_refresh(
    providers: &Entity<FrontInsertMap<UniqueId, Arc<Provider>>>,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    cx: &mut App,
) {
    cx.observe(providers, move |_, cx| {
        state.items.update(cx, |items, cx| {
            *items = SelectItemsMap::new();
            cx.notify();
        });
    })
    .detach();
}
