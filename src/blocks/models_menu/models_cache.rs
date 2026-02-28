use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::SharedString;
use tracing::info;

use schema::UniqueId;

const MODEL_FETCH_COOLDOWN_SECS: u64 = 120;

#[derive(Clone)]
pub struct CachedModel {
    pub provider_id: UniqueId,
    pub provider_name: String,
    pub model_id: String,
    pub display_name: String,
    pub parameters: Option<String>,
    pub quantization: Option<String>,
    pub icon_path: SharedString,
    pub has_thinking: bool,
}

struct ProviderModels {
    /// (model_id, display_name, parameters, quantization, has_thinking)
    models: Vec<(String, String, Option<String>, Option<String>, bool)>,
    provider_name: String,
    icon_path: SharedString,
    fetched_at: Instant,
}

pub struct ModelsCache {
    all_models: Vec<CachedModel>,
    per_provider: HashMap<UniqueId, ProviderModels>,
    pub(super) provider_config_cache: HashMap<UniqueId, CachedProviderState>,
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

    pub fn model_supports_thinking(&self, provider_id: &UniqueId, model_id: &str) -> bool {
        self.all_models.iter().any(|m| {
            &m.provider_id == provider_id && m.model_id == model_id && m.has_thinking
        })
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
    ) -> Option<(&str, &[(String, String, Option<String>, Option<String>, bool)])> {
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
        icon_path: SharedString,
        models: Vec<(String, String, Option<String>, Option<String>, bool)>,
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
                icon_path,
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
    /// If no cache exists, creates one with the current URL.
    pub(super) fn get_or_create_config_cache(
        &mut self,
        provider_id: &UniqueId,
        current_url: &str,
    ) -> &mut CachedProviderState {
        self.provider_config_cache
            .entry(provider_id.clone())
            .or_insert_with(|| CachedProviderState::new(current_url))
    }

    fn rebuild_all_models(&mut self) {
        self.all_models.clear();
        for (provider_id, provider_models) in &self.per_provider {
            for (model_id, display_name, parameters, quantization, has_thinking) in &provider_models.models {
                self.all_models.push(CachedModel {
                    provider_id: provider_id.clone(),
                    provider_name: provider_models.provider_name.clone(),
                    model_id: model_id.clone(),
                    display_name: display_name.clone(),
                    parameters: parameters.clone(),
                    quantization: quantization.clone(),
                    icon_path: provider_models.icon_path.clone(),
                    has_thinking: *has_thinking,
                });
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct CachedProviderState {
    url: String,
}

impl CachedProviderState {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
        }
    }

    pub fn url_changed(&self, new_url: &str) -> bool {
        self.url != new_url
    }

    pub fn set_url(&mut self, url: impl Into<String>) {
        self.url = url.into();
    }
}
