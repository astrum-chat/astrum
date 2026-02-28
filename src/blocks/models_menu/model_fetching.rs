use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{App, AppContext, AsyncApp, Entity, SharedString};
use gpui_tesserae::components::select::{SelectItem, SelectItemsMap, SelectState};
use tracing::{debug, error};

use schema::UniqueId;

use crate::managers::{Managers, ModelsManager, Provider};
use crate::utils::FrontInsertMap;

use super::models_cache::ModelsCache;
use super::{ModelSelectItem, ModelSelection, ModelSelectionSource};

static FETCH_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

struct FetchGuard;

impl FetchGuard {
    fn try_acquire() -> Option<Self> {
        FETCH_IN_PROGRESS
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| FetchGuard)
    }
}

impl Drop for FetchGuard {
    fn drop(&mut self) {
        FETCH_IN_PROGRESS.store(false, Ordering::Release);
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
    managers: Managers,
    provider_id: UniqueId,
    config_change: ProviderConfigChange,
    cx: &mut App,
) {
    let models_cache = managers.models.read(cx).models_cache.clone();

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
    managers: &Managers,
    models_cache: &Entity<ModelsCache>,
    provider_id: &UniqueId,
    config_change: &ProviderConfigChange,
    cx: &mut App,
) -> bool {
    let current_url = managers.models.read_with(cx, |models, cx| {
        models.providers
            .read(cx)
            .get(provider_id)
            .map(|p| p.url.read(cx).to_string())
            .unwrap_or_default()
    });

    models_cache.update(cx, |cache, _| {
        let config_cache =
            cache.get_or_create_config_cache(provider_id, &current_url);

        match config_change {
            ProviderConfigChange::Create => true,
            ProviderConfigChange::Url(url) => {
                let changed = config_cache.url_changed(url);
                if changed {
                    config_cache.set_url(url);
                }
                changed
            }
            // API key changes always proceed — no caching of secrets
            ProviderConfigChange::ApiKey(_) => true,
        }
    })
}

fn apply_config_change(
    managers: &Managers,
    provider_id: &UniqueId,
    config_change: &ProviderConfigChange,
    cx: &mut App,
) {
    managers.models.update(cx, |models, cx| {
        match config_change {
            ProviderConfigChange::Create => {}
            ProviderConfigChange::Url(url) => {
                let _ = models
                    .edit_provider_url(cx, provider_id.clone(), url.clone());
            }
            ProviderConfigChange::ApiKey(api_key) => {
                let _ = models.edit_provider_api_key(
                    cx,
                    provider_id.clone(),
                    api_key.clone(),
                );
            }
        }
    });

    let models = managers.models.clone();
    let provider_id = provider_id.clone();
    cx.spawn(async move |cx: &mut AsyncApp| {
        ModelsManager::reinit_provider(models, provider_id, cx).await;
    })
    .detach();
}

fn spawn_fetch_models(
    managers: Managers,
    provider_id: UniqueId,
    models_cache: Entity<ModelsCache>,
    cx: &mut App,
) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let provider: Option<crate::managers::Provider> = cx.read_entity(&managers.models, |models, cx| {
            models.providers
                .read(cx)
                .get(&provider_id)
                .map(|p| p.as_ref().clone())
        });

        let Some(provider) = provider else {
            return;
        };

        let provider_name: String =
            cx.read_entity(&provider.name, |name: &SharedString, _| name.to_string());
        let icon_path: SharedString =
            cx.read_entity(&provider.icon, |icon: &SharedString, _| icon.clone());

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
                    cache.refresh_models_for_provider(
                        provider_id,
                        provider_name,
                        icon_path,
                        model_pairs,
                    );
                });
            }
            Err(err) => {
                error!(provider_name = %provider_name, error = %err, "Failed to fetch models from provider");
            }
        }
    })
    .detach();
}

pub fn prefetch_all_models(managers: Managers, cx: &mut App) {
    let Some(guard) = FetchGuard::try_acquire() else {
        return;
    };

    debug!("Prefetching models from all providers");

    cx.spawn(async move |cx: &mut AsyncApp| {
        let _guard = guard;
        let (providers_info, models_cache): (
            Vec<(UniqueId, crate::managers::Provider)>,
            Entity<ModelsCache>,
        ) = cx.read_entity(&managers.models, |models, cx| {
            let models_cache = models.models_cache.clone();

            let providers = models.providers
                .read(cx)
                .iter()
                .map(|(id, p)| (id.clone(), p.as_ref().clone()))
                .collect::<Vec<(UniqueId, crate::managers::Provider)>>();

            (providers, models_cache)
        });

        for (provider_id, provider) in providers_info {
            let provider_name: String =
                cx.read_entity(&provider.name, |name: &SharedString, _| name.to_string());
            let icon_path: SharedString =
                cx.read_entity(&provider.icon, |icon: &SharedString, _| icon.clone());

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
                    let icon_path_clone = icon_path.clone();

                    let _ = models_cache.update(cx, |cache, _| {
                        cache.refresh_models_for_provider(
                            provider_id_clone,
                            provider_name_clone,
                            icon_path_clone,
                            model_pairs,
                        );
                    });
                }
                Err(err) => {
                error!(provider_name = %provider_name, error = %err, "Failed to fetch models from provider");
            }
            }
        }

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
    icon_path: SharedString,
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
        icon_path,
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
    managers: Managers,
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
    managers: Managers,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    models_cache: Entity<ModelsCache>,
    source: ModelSelectionSource,
    cx: &mut App,
) {
    let Some(guard) = FetchGuard::try_acquire() else {
        return;
    };

    let (current_provider_id, current_model): (Option<UniqueId>, Option<String>) = managers.models.read_with(cx, |models, cx| {
        let pair = match source {
            ModelSelectionSource::Current => &models.current_model,
            ModelSelectionSource::ChatTitles => &models.chat_titles_model,
        };
        let (provider_id, _, model, _, _) = pair.read_selection(cx);
        (provider_id, model)
    });

    cx.spawn(async move |cx: &mut AsyncApp| {
        let _guard = guard;
        let providers_info: Vec<(UniqueId, crate::managers::Provider)> = cx.read_entity(&managers.models, |models, cx| {
            models.providers
                .read(cx)
                .iter()
                .map(|(id, p)| (id.clone(), p.as_ref().clone()))
                .collect::<Vec<(UniqueId, crate::managers::Provider)>>()
        });

        for (provider_id, provider) in providers_info {
            let provider_name: String =
                cx.read_entity(&provider.name, |name: &SharedString, _| name.to_string());
            let icon_path: SharedString =
                cx.read_entity(&provider.icon, |icon: &SharedString, _| icon.clone());

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
                    let icon_path = icon_path.clone();
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
                                icon_path.clone(),
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
                    let icon_path_clone = icon_path.clone();

                    let _ = models_cache.update(cx, |cache, _| {
                        cache.refresh_models_for_provider(
                            provider_id_clone,
                            provider_name_clone,
                            icon_path_clone,
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
                                icon_path.clone(),
                                current_provider_id.as_ref(),
                                current_model.as_ref(),
                                cx,
                            );
                        }
                    });
                }
                Err(err) => {
                error!(provider_name = %provider_name, error = %err, "Failed to fetch models from provider");
            }
            }
        }

    })
    .detach();
}

pub fn observe_providers_for_refresh(
    providers: &Entity<FrontInsertMap<UniqueId, Arc<Provider>>>,
    state: Arc<SelectState<ModelSelection, ModelSelectItem>>,
    managers: Managers,
    cx: &mut App,
) {
    cx.observe(providers, move |providers, cx| {
        state.items.update(cx, |items, cx| {
            *items = SelectItemsMap::new();
            cx.notify();
        });

        state.remove_selection(cx);

        managers.models.update(cx, |models, cx| {
            let current_provider_id = models.current_model.provider_id.read(cx).clone();

            if let Some(provider_id) = current_provider_id {
                let provider_exists = providers.read(cx).get(&provider_id).is_some();
                if !provider_exists {
                    models.clear_current_selection(cx);
                }
            }

            let chat_titles_provider_id = models
                .chat_titles_model
                .provider_id
                .read(cx)
                .clone();

            if let Some(provider_id) = chat_titles_provider_id {
                let provider_exists = providers.read(cx).get(&provider_id).is_some();
                if !provider_exists {
                    models.clear_chat_titles_selection(cx);
                }
            }
        });
    })
    .detach();
}
