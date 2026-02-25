use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use tracing::{debug, error, info};

use gpui::{
    App, AsyncApp, ElementId, Entity, Hsla, IntoElement, SharedString, Window, div, prelude::*,
};
use gpui_tesserae::{
    ElementIdExt,
    components::select::{SelectItem, SelectItemsMap, SelectState},
};
use smol::lock::RwLock;

use anyml::models::{Model, ModelParams, ModelQuant};

use schema::UniqueId;
use crate::{Managers, managers::Provider, utils::FrontInsertMap};

const MODEL_FETCH_COOLDOWN_SECS: u64 = 120;

static FETCH_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
pub struct CachedModel {
    pub provider_id: UniqueId,
    pub provider_name: String,
    pub model_id: String,
    pub display_name: String,
    pub parameters: Option<String>,
    pub quantization: Option<String>,
}

struct ProviderModels {
    /// (model_id, display_name, parameters, quantization)
    models: Vec<(String, String, Option<String>, Option<String>)>,
    provider_name: String,
    fetched_at: Instant,
}

pub struct ModelsCache {
    all_models: Vec<CachedModel>,
    per_provider: HashMap<UniqueId, ProviderModels>,
    provider_config_cache: HashMap<UniqueId, CachedProviderState>,
}

impl ModelsCache {
    pub fn new() -> Self {
        Self {
            all_models: Vec::new(),
            per_provider: HashMap::new(),
            provider_config_cache: HashMap::new(),
        }
    }

    pub fn get_all_models(&self) -> &[CachedModel] {
        &self.all_models
    }

    pub fn is_provider_stale(&self, provider_id: &UniqueId) -> bool {
        match self.per_provider.get(provider_id) {
            Some(cached) => {
                cached.fetched_at.elapsed() >= Duration::from_secs(MODEL_FETCH_COOLDOWN_SECS)
            }
            None => true,
        }
    }

    pub fn get_provider_models(
        &self,
        provider_id: &UniqueId,
    ) -> Option<(&str, &[(String, String, Option<String>, Option<String>)])> {
        let cached = self.per_provider.get(provider_id)?;
        if cached.fetched_at.elapsed() < Duration::from_secs(MODEL_FETCH_COOLDOWN_SECS) {
            Some((&cached.provider_name, &cached.models))
        } else {
            None
        }
    }

    pub fn refresh_models_for_provider(
        &mut self,
        provider_id: UniqueId,
        provider_name: String,
        models: Vec<(String, String, Option<String>, Option<String>)>,
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

    pub fn delete_models_for_provider(&mut self, provider_id: &UniqueId) {
        if let Some(removed) = self.per_provider.remove(provider_id) {
            info!(
                provider_name = %removed.provider_name,
                provider_id = %provider_id,
                "Invalidated cache for provider"
            );
        }
        self.provider_config_cache.remove(provider_id);
        self.rebuild_all_models();
    }

    /// Get or create cached config state for a provider.
    /// If no cache exists, creates one with the current values.
    fn get_or_create_config_cache(
        &mut self,
        provider_id: &UniqueId,
        current_url: &str,
        current_api_key: &str,
    ) -> &mut CachedProviderState {
        self.provider_config_cache
            .entry(provider_id.clone())
            .or_insert_with(|| CachedProviderState::new(current_url, current_api_key))
    }

    fn rebuild_all_models(&mut self) {
        self.all_models.clear();
        for (provider_id, provider_models) in &self.per_provider {
            for (model_id, display_name, parameters, quantization) in &provider_models.models {
                self.all_models.push(CachedModel {
                    provider_id: provider_id.clone(),
                    provider_name: provider_models.provider_name.clone(),
                    model_id: model_id.clone(),
                    display_name: display_name.clone(),
                    parameters: parameters.clone(),
                    quantization: quantization.clone(),
                });
            }
        }
    }
}

fn hash_api_key(api_key: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    hasher.finalize().into()
}

#[derive(Clone)]
struct CachedProviderState {
    url: String,
    api_key_hash: [u8; 32],
}

impl CachedProviderState {
    pub fn new(url: impl Into<String>, api_key: &str) -> Self {
        Self {
            url: url.into(),
            api_key_hash: hash_api_key(api_key),
        }
    }

    pub fn url_changed(&self, new_url: &str) -> bool {
        self.url != new_url
    }

    pub fn api_key_changed(&self, new_api_key: &str) -> bool {
        self.api_key_hash != hash_api_key(new_api_key)
    }

    pub fn set_url(&mut self, url: impl Into<String>) {
        self.url = url.into();
    }

    pub fn set_api_key(&mut self, api_key: &str) {
        self.api_key_hash = hash_api_key(api_key);
    }
}

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
    ) -> Self {
        Self {
            display_name: display_name.to_string().into(),
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
            .text_ellipsis()
            .text_color(text_color)
            .child(self.name())
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
        let display_name = Model {
            id: selection.model_id.clone(),
            parameters: selection.parameters.as_deref().map(|p| ModelParams::new(p)),
            quantization: selection
                .quantization
                .as_deref()
                .map(|q| ModelQuant::new(q)),
        }
        .to_string();
        let item = ModelSelectItem::new(
            &selection.provider_name,
            selection.model_id.clone(),
            &display_name,
            selection.provider_id,
            selection.parameters,
            selection.quantization,
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
                managers.models.set_current_selection(
                    cx,
                    selection.provider_id,
                    selection.provider_name,
                    selection.model_id,
                    selection.parameters,
                    selection.quantization,
                );
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

/// Reason for refetching provider models.
pub enum ProviderConfigChange {
    /// New provider created - skip change detection
    Create,
    /// URL may have changed
    Url(String),
    /// API key may have changed (None = cleared)
    ApiKey(Option<String>),
}

/// Refetches models for a provider. For `Url` and `ApiKey` changes, checks if
/// the value actually changed before applying and refetching.
pub fn refetch_provider_models(
    managers: Arc<RwLock<Managers>>,
    provider_id: UniqueId,
    config_change: ProviderConfigChange,
    cx: &mut App,
) {
    let models_cache = managers.read_arc_blocking().models.models_cache.clone();

    if !matches!(config_change, ProviderConfigChange::Create) {
        let should_proceed = check_and_update_config_cache(
            &managers,
            &models_cache,
            &provider_id,
            &config_change,
            cx,
        );

        if !should_proceed {
            return;
        }

        apply_config_change(&managers, &provider_id, &config_change, cx);
    }

    spawn_fetch_models(managers, provider_id, models_cache, cx);
}

fn check_and_update_config_cache(
    managers: &Arc<RwLock<Managers>>,
    models_cache: &Entity<ModelsCache>,
    provider_id: &UniqueId,
    config_change: &ProviderConfigChange,
    cx: &mut App,
) -> bool {
    let managers_guard = managers.read_arc_blocking();

    let current_url = cx
        .read_entity(&managers_guard.models.providers, |providers, _cx| {
            providers
                .get(provider_id)
                .map(|p| p.url.read(_cx).to_string())
        })
        .unwrap_or_default();

    let current_api_key = managers_guard
        .models
        .get_provider_api_key(cx, provider_id)
        .unwrap_or_default();

    models_cache.update(cx, |cache, _| {
        let config_cache =
            cache.get_or_create_config_cache(provider_id, &current_url, &current_api_key);

        match config_change {
            ProviderConfigChange::Create => true,
            ProviderConfigChange::Url(url) => {
                let changed = config_cache.url_changed(url);
                if changed {
                    config_cache.set_url(url);
                }
                changed
            }
            ProviderConfigChange::ApiKey(api_key) => {
                let key = api_key.as_deref().unwrap_or("");
                let changed = config_cache.api_key_changed(key);
                if changed {
                    config_cache.set_api_key(key);
                }
                changed
            }
        }
    })
}

fn apply_config_change(
    managers: &Arc<RwLock<Managers>>,
    provider_id: &UniqueId,
    config_change: &ProviderConfigChange,
    cx: &mut App,
) {
    let mut managers_guard = managers.write_arc_blocking();
    match config_change {
        ProviderConfigChange::Create => {}
        ProviderConfigChange::Url(url) => {
            let _ = managers_guard
                .models
                .edit_provider_url(cx, provider_id.clone(), url.clone());
        }
        ProviderConfigChange::ApiKey(api_key) => {
            let _ = managers_guard.models.edit_provider_api_key(
                cx,
                provider_id.clone(),
                api_key.clone(),
            );
        }
    }
    let _ = managers_guard.models.reinit_provider(cx, provider_id);
}

fn spawn_fetch_models(
    managers: Arc<RwLock<Managers>>,
    provider_id: UniqueId,
    models_cache: Entity<ModelsCache>,
    cx: &mut App,
) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let provider: Option<crate::managers::Provider> = {
            let managers = managers.read_arc_blocking();

            cx.read_entity(&managers.models.providers, |providers, _cx| {
                providers.get(&provider_id).map(|p| p.as_ref().clone())
            })
        };

        let Some(provider) = provider else {
            return;
        };

        let provider_name: String =
            cx.read_entity(&provider.name, |name: &SharedString, _| name.to_string());

        debug!(
            provider_name = %provider_name,
            provider_id = %provider_id,
            "Fetching models for provider"
        );

        match provider.inner.list_models().await {
            Ok(models) => {
                let model_pairs: Vec<(String, String, Option<String>, Option<String>)> = models
                    .iter()
                    .map(|m| {
                        (
                            m.id.clone(),
                            m.to_string(),
                            m.parameters.as_ref().map(|p| p.as_str().to_string()),
                            m.quantization.as_ref().map(|q| q.as_str().to_string()),
                        )
                    })
                    .collect();

                let _ = models_cache.update(cx, |cache, _| {
                    cache.refresh_models_for_provider(provider_id, provider_name, model_pairs);
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

pub fn prefetch_all_models(managers: Arc<RwLock<Managers>>, cx: &mut App) {
    if FETCH_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return;
    }

    debug!("Prefetching models from all providers");

    cx.spawn(async move |cx: &mut AsyncApp| {
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

            match provider.inner.list_models().await {
                Ok(models) => {
                    let model_pairs: Vec<(String, String, Option<String>, Option<String>)> = models
                        .iter()
                        .map(|m| {
                            (
                                m.id.clone(),
                                m.to_string(),
                                m.parameters.as_ref().map(|p| p.as_str().to_string()),
                                m.quantization.as_ref().map(|q| q.as_str().to_string()),
                            )
                        })
                        .collect();
                    let provider_name_clone = provider_name.clone();
                    let provider_id_clone = provider_id.clone();

                    let _ = models_cache.update(cx, |cache, _| {
                        cache.refresh_models_for_provider(
                            provider_id_clone,
                            provider_name_clone,
                            model_pairs,
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

        FETCH_IN_PROGRESS.store(false, Ordering::SeqCst);
        debug!("Prefetch complete");
    })
    .detach();
}

fn push_model_item(
    state: &Arc<SelectState<ModelSelection, ModelSelectItem>>,
    provider_name: &str,
    model_id: &str,
    display_name: &str,
    provider_id: &UniqueId,
    parameters: Option<String>,
    quantization: Option<String>,
    current_provider_id: Option<&UniqueId>,
    current_model: Option<&String>,
    cx: &mut App,
) {
    let item = ModelSelectItem::new(
        provider_name,
        model_id.to_string(),
        display_name,
        provider_id.clone(),
        parameters,
        quantization,
    );
    let item_name = item.name();
    state.push_item(cx, item);

    if current_provider_id == Some(provider_id)
        && current_model.map(|s| s.as_str()) == Some(model_id)
    {
        let _ = state.select_item(cx, item_name);
    }
}

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

pub fn fetch_all_models_with_source(
    managers: Arc<RwLock<Managers>>,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    models_cache: Entity<ModelsCache>,
    source: ModelSelectionSource,
    cx: &mut App,
) {
    if FETCH_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return;
    }

    let (current_provider_id, current_model): (Option<UniqueId>, Option<String>) = {
        let managers = managers.read_arc_blocking();
        let pair = match source {
            ModelSelectionSource::Current => &managers.models.current_model,
            ModelSelectionSource::ChatTitles => &managers.models.chat_titles_model,
        };
        let (provider_id, _, model, _, _) = pair.read_selection(cx);
        (provider_id, model)
    };

    cx.spawn(async move |cx: &mut AsyncApp| {
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

            let is_stale = cx.read_entity(&models_cache, |cache, _| {
                cache.is_provider_stale(&provider_id)
            });

            if !is_stale {
                let cached_models: Option<Vec<(String, String, Option<String>, Option<String>)>> =
                    cx.read_entity(&models_cache, |cache, _| {
                        cache
                            .get_provider_models(&provider_id)
                            .map(|(_, models)| models.to_vec())
                    });

                if let Some(models) = cached_models {
                    let _ = cx.update(|cx| {
                        for (model_id, display_name, parameters, quantization) in &models {
                            push_model_item(
                                &state,
                                &provider_name,
                                model_id,
                                display_name,
                                &provider_id,
                                parameters.clone(),
                                quantization.clone(),
                                current_provider_id.as_ref(),
                                current_model.as_ref(),
                                cx,
                            );
                        }
                    });
                    continue;
                }
            }

            match provider.inner.list_models().await {
                Ok(models) => {
                    let model_pairs: Vec<(String, String, Option<String>, Option<String>)> = models
                        .iter()
                        .map(|m| {
                            (
                                m.id.clone(),
                                m.to_string(),
                                m.parameters.as_ref().map(|p| p.as_str().to_string()),
                                m.quantization.as_ref().map(|q| q.as_str().to_string()),
                            )
                        })
                        .collect();
                    let provider_name_clone = provider_name.clone();
                    let provider_id_clone = provider_id.clone();

                    let _ = models_cache.update(cx, |cache, _| {
                        cache.refresh_models_for_provider(
                            provider_id_clone,
                            provider_name_clone,
                            model_pairs,
                        );
                    });

                    let _ = cx.update(|cx| {
                        for model in &models {
                            push_model_item(
                                &state,
                                &provider_name,
                                &model.id,
                                &model.to_string(),
                                &provider_id,
                                model.parameters.as_ref().map(|p| p.as_str().to_string()),
                                model.quantization.as_ref().map(|q| q.as_str().to_string()),
                                current_provider_id.as_ref(),
                                current_model.as_ref(),
                                cx,
                            );
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

        FETCH_IN_PROGRESS.store(false, Ordering::SeqCst);
    })
    .detach();
}

pub fn observe_providers_for_refresh(
    providers: &Entity<FrontInsertMap<UniqueId, Arc<Provider>>>,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    managers: Arc<RwLock<Managers>>,
    cx: &mut App,
) {
    cx.observe(providers, move |providers, cx| {
        state.items.update(cx, |items, cx| {
            *items = SelectItemsMap::new();
            cx.notify();
        });

        state.remove_selection(cx);

        let mut managers = managers.write_arc_blocking();
        let current_provider_id = managers.models.current_model.provider_id.read(cx).clone();

        if let Some(provider_id) = current_provider_id {
            let provider_exists = providers.read(cx).get(&provider_id).is_some();
            if !provider_exists {
                managers.models.clear_current_selection(cx);
            }
        }

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
