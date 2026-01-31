use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tracing::{debug, error, info};

use gpui::{
    App, AsyncApp, ElementId, Entity, Hsla, IntoElement, SharedString, Window, div, prelude::*,
};
use gpui_tesserae::{
    ElementIdExt,
    components::select::{SelectItem, SelectItemsMap, SelectState},
};
use smol::lock::RwLock;

use crate::{Managers, managers::Provider, managers::UniqueId, utils::FrontInsertMap};

/// Minimum interval between model fetches per provider (in seconds)
const MODEL_FETCH_COOLDOWN_SECS: u64 = 120;

/// Global flag to prevent concurrent fetch operations
static FETCH_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// A cached model entry with provider info
#[derive(Clone)]
pub struct CachedModel {
    pub provider_id: UniqueId,
    pub provider_name: String,
    pub model_id: String,
}

/// Per-provider model cache entry
struct ProviderModels {
    models: Vec<String>,
    provider_name: String,
    fetched_at: Instant,
}

/// Cache for provider models with per-provider granularity
pub struct ModelsCache {
    /// All models concatenated for easy iteration
    all_models: Vec<CachedModel>,
    /// Per-provider model lists with fetch timestamps
    per_provider: HashMap<UniqueId, ProviderModels>,
}

impl ModelsCache {
    pub fn new() -> Self {
        Self {
            all_models: Vec::new(),
            per_provider: HashMap::new(),
        }
    }

    /// Get all cached models (flat list)
    pub fn get_all_models(&self) -> &[CachedModel] {
        &self.all_models
    }

    /// Check if provider cache is stale (> cooldown or missing)
    pub fn is_provider_stale(&self, provider_id: &UniqueId) -> bool {
        match self.per_provider.get(provider_id) {
            Some(cached) => {
                cached.fetched_at.elapsed() >= Duration::from_secs(MODEL_FETCH_COOLDOWN_SECS)
            }
            None => true,
        }
    }

    /// Get cached models for a specific provider (if fresh)
    pub fn get_provider_models(&self, provider_id: &UniqueId) -> Option<(&str, &[String])> {
        let cached = self.per_provider.get(provider_id)?;
        if cached.fetched_at.elapsed() < Duration::from_secs(MODEL_FETCH_COOLDOWN_SECS) {
            Some((&cached.provider_name, &cached.models))
        } else {
            None
        }
    }

    /// Update models for a specific provider, rebuilds all_models
    pub fn refresh_models_for_provider(
        &mut self,
        provider_id: UniqueId,
        provider_name: String,
        models: Vec<String>,
    ) {
        info!(
            provider_name = %provider_name,
            provider_id = %provider_id,
            model_count = models.len(),
            "Refreshed models for provider"
        );
        self.per_provider.insert(
            provider_id,
            ProviderModels {
                models,
                provider_name,
                fetched_at: Instant::now(),
            },
        );
        self.rebuild_all_models();
    }

    /// Remove a provider's models, rebuilds all_models
    pub fn delete_models_for_provider(&mut self, provider_id: &UniqueId) {
        if let Some(removed) = self.per_provider.remove(provider_id) {
            info!(
                provider_name = %removed.provider_name,
                provider_id = %provider_id,
                "Invalidated cache for provider"
            );
        }
        self.rebuild_all_models();
    }

    /// Rebuild the flat all_models list from per_provider data
    fn rebuild_all_models(&mut self) {
        self.all_models.clear();
        for (provider_id, provider_models) in &self.per_provider {
            for model_id in &provider_models.models {
                self.all_models.push(CachedModel {
                    provider_id: provider_id.clone(),
                    provider_name: provider_models.provider_name.clone(),
                    model_id: model_id.clone(),
                });
            }
        }
    }
}

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
            cached.provider_id.clone(),
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

/// Refetches models for a single provider and updates the cache.
/// Called when a provider's configuration (URL, API key) changes.
pub fn refetch_provider_models(
    managers: Arc<RwLock<Managers>>,
    provider_id: UniqueId,
    cx: &mut App,
) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let (provider, models_cache): (Option<crate::managers::Provider>, Entity<ModelsCache>) = {
            let managers = managers.read_arc_blocking();
            let models_cache = managers.models.models_cache.clone();

            let provider = cx.read_entity(&managers.models.providers, |providers, _cx| {
                providers.get(&provider_id).map(|p| p.as_ref().clone())
            });

            (provider, models_cache)
        };

        let Some(provider) = provider else {
            return;
        };

        let provider_name: String =
            cx.read_entity(&provider.name, |name: &SharedString, _| name.to_string());

        debug!(
            provider_name = %provider_name,
            provider_id = %provider_id,
            "Refetching models for provider"
        );

        match provider.inner.list_models().await {
            Ok(models) => {
                let model_ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();

                let _ = models_cache.update(cx, |cache, _| {
                    cache.refresh_models_for_provider(provider_id, provider_name, model_ids);
                });
            }
            Err(err) => {
                error!(
                    provider_name = %provider_name,
                    provider_id = %provider_id,
                    error = %err,
                    "Failed to refetch models for provider"
                );
            }
        }
    })
    .detach();
}

/// Prefetches models from all providers into the cache on startup.
/// This populates the cache so models are immediately available when the picker is opened.
pub fn prefetch_all_models(managers: Arc<RwLock<Managers>>, cx: &mut App) {
    // Prevent concurrent fetch operations
    if FETCH_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return;
    }

    debug!("Prefetching models from all providers");

    cx.spawn(async move |cx: &mut AsyncApp| {
        // Collect provider info we need for fetching
        let (providers_info, models_cache): (
            Vec<(UniqueId, crate::managers::Provider)>,
            Entity<ModelsCache>,
        ) = {
            let managers = managers.read_arc_blocking();
            let models_cache = managers.models.models_cache.clone();

            let providers = cx.read_entity(&managers.models.providers, |providers, _cx| {
                providers
                    .iter()
                    .map(|(id, p)| (id.clone(), p.as_ref().clone()))
                    .collect::<Vec<(UniqueId, crate::managers::Provider)>>()
            });

            (providers, models_cache)
        };

        for (provider_id, provider) in providers_info {
            let provider_name: String =
                cx.read_entity(&provider.name, |name: &SharedString, _| name.to_string());

            // Fetch from API
            match provider.inner.list_models().await {
                Ok(models) => {
                    let model_ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();
                    let provider_name_clone = provider_name.clone();
                    let provider_id_clone = provider_id.clone();

                    let _ = models_cache.update(cx, |cache, _| {
                        cache.refresh_models_for_provider(
                            provider_id_clone,
                            provider_name_clone,
                            model_ids,
                        );
                    });
                }
                Err(err) => {
                    error!(
                        provider_name = %provider_name,
                        provider_id = %provider_id,
                        error = %err,
                        "Failed to prefetch models from provider"
                    );
                }
            }
        }

        // Reset fetch in progress flag
        FETCH_IN_PROGRESS.store(false, Ordering::SeqCst);
        debug!("Prefetch complete");
    })
    .detach();
}

/// Fetches models from all providers asynchronously and populates the select state.
pub fn fetch_all_models(
    managers: Arc<RwLock<Managers>>,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    models_cache: Entity<ModelsCache>,
    cx: &mut App,
) {
    fetch_all_models_with_source(
        managers,
        state,
        models_cache,
        ModelSelectionSource::Current,
        cx,
    );
}

/// Fetches models from all providers asynchronously and populates the select state.
/// Uses the specified source to determine which model to auto-select.
/// Models are cached per-provider to avoid excessive API calls.
pub fn fetch_all_models_with_source(
    managers: Arc<RwLock<Managers>>,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    models_cache: Entity<ModelsCache>,
    source: ModelSelectionSource,
    cx: &mut App,
) {
    // Prevent concurrent fetch operations
    if FETCH_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return;
    }

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
            let provider_name: String =
                cx.read_entity(&provider.name, |name: &SharedString, _| name.to_string());

            // Check cache first
            let is_stale = cx.read_entity(&models_cache, |cache, _| {
                cache.is_provider_stale(&provider_id)
            });

            if !is_stale {
                // Use cached models
                let cached_models: Option<Vec<String>> =
                    cx.read_entity(&models_cache, |cache, _| {
                        cache
                            .get_provider_models(&provider_id)
                            .map(|(_, models)| models.to_vec())
                    });

                if let Some(models) = cached_models {
                    let _ = cx.update(|cx| {
                        for model_id in models {
                            let item = ModelSelectItem::new(
                                &provider_name,
                                model_id.clone(),
                                provider_id.clone(),
                            );

                            let item_name = item.name();
                            state.push_item(cx, item);

                            let provider_matches =
                                current_provider_id.as_ref() == Some(&provider_id);
                            let model_matches = current_model.as_ref() == Some(&model_id);

                            if provider_matches && model_matches {
                                let _ = state.select_item(cx, item_name);
                            }
                        }
                    });
                    continue;
                }
            }

            // Fetch from API if not cached or stale
            match provider.inner.list_models().await {
                Ok(models) => {
                    // Cache the model IDs
                    let model_ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();
                    let provider_name_clone = provider_name.clone();
                    let provider_id_clone = provider_id.clone();

                    let _ = models_cache.update(cx, |cache, _| {
                        cache.refresh_models_for_provider(
                            provider_id_clone,
                            provider_name_clone,
                            model_ids,
                        );
                    });

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
                    error!(
                        provider_name = %provider_name,
                        error = %err,
                        "Failed to fetch models from provider"
                    );
                }
            }
        }

        // Reset fetch in progress flag
        FETCH_IN_PROGRESS.store(false, Ordering::SeqCst);
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
