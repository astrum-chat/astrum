use std::collections::VecDeque;
use std::sync::Arc;

use anyml::{
    AnthropicProvider, OllamaProvider, OpenAiProvider,
    providers::{chat::ChatProvider, list_models::ListModelsProvider},
};
use enum_assoc::Assoc;
use gpui::{App, AppContext, AsyncApp, Entity, SharedString, Task};
use notitia::prelude::*;
use notitia::{Notitia, PrimaryKey};
use notitia_sqlite::SqliteAdapter;
use secrecy::{ExposeSecret, SecretString};
use tracing::error;

pub trait ProviderTrait: ChatProvider + ListModelsProvider {}
impl<T: ChatProvider + ListModelsProvider> ProviderTrait for T {}

use schema::{AstrumDb, DbDateTime, ModelSelectionRecord, ProviderRecord, UniqueId};

use crate::{
    anyhttp_gpui::GpuiHttpWrapper,
    assets::{AstrumProviderIconKind, AstrumProviderLogoKind},
    blocks::models_menu::ModelsCache,
    secrets::{get_secret, remove_secret, set_secret},
    utils::FrontInsertMap,
};

pub struct ProviderModelPair {
    pub provider_id: Entity<Option<UniqueId>>,
    pub provider_name: Entity<Option<String>>,
    pub model: Entity<Option<String>>,
    pub parameters: Entity<Option<String>>,
    pub quantization: Entity<Option<String>>,
}

impl ProviderModelPair {
    pub fn read_selection(
        &self,
        cx: &App,
    ) -> (
        Option<UniqueId>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) {
        (
            self.provider_id.read(cx).clone(),
            self.provider_name.read(cx).clone(),
            self.model.read(cx).clone(),
            self.parameters.read(cx).clone(),
            self.quantization.read(cx).clone(),
        )
    }
}

pub struct ModelsManager {
    db: Option<Notitia<AstrumDb, SqliteAdapter>>,
    pub providers: Entity<FrontInsertMap<UniqueId, Arc<Provider>>>,
    pub current_model: ProviderModelPair,
    pub chat_titles_model: ProviderModelPair,
    pub models_cache: Entity<ModelsCache>,
}

impl<'a> ModelsManager {
    pub fn new(cx: &mut App) -> Self {
        Self {
            db: None,
            providers: cx.new(move |_cx| FrontInsertMap::new()),
            current_model: ProviderModelPair {
                provider_id: cx.new(|_cx| None),
                provider_name: cx.new(|_cx| None),
                model: cx.new(|_cx| None),
                parameters: cx.new(|_cx| None),
                quantization: cx.new(|_cx| None),
            },
            chat_titles_model: ProviderModelPair {
                provider_id: cx.new(|_cx| None),
                provider_name: cx.new(|_cx| None),
                model: cx.new(|_cx| None),
                parameters: cx.new(|_cx| None),
                quantization: cx.new(|_cx| None),
            },
            models_cache: cx.new(|_cx| ModelsCache::new()),
        }
    }

    pub fn init(&mut self, _cx: &mut App, db: Notitia<AstrumDb, SqliteAdapter>) {
        self.db = Some(db);
    }

    fn db(&self) -> &Notitia<AstrumDb, SqliteAdapter> {
        self.db.as_ref().expect("ModelsManager not initialized")
    }

    pub fn get_current_provider<'b>(&'b self, cx: &'b App) -> Option<&'b Arc<Provider>> {
        self.current_model
            .provider_id
            .read(cx)
            .as_ref()
            .and_then(|id| self.providers.read(cx).get(id))
    }

    pub fn set_current_selection(
        &mut self,
        cx: &mut App,
        provider_id: UniqueId,
        provider_name: impl Into<String>,
        model: impl Into<String>,
        parameters: Option<String>,
        quantization: Option<String>,
    ) {
        let provider_name = provider_name.into();
        let model = model.into();

        cx.update_entity(
            &self.current_model.provider_id,
            |current_provider_id, cx| {
                *current_provider_id = Some(provider_id.clone());
                cx.notify();
            },
        );

        cx.update_entity(
            &self.current_model.provider_name,
            |current_provider_name, cx| {
                *current_provider_name = Some(provider_name.clone());
                cx.notify();
            },
        );

        cx.update_entity(&self.current_model.model, |current_model, cx| {
            *current_model = Some(model.clone());
            cx.notify();
        });

        cx.update_entity(&self.current_model.parameters, |current_params, cx| {
            *current_params = parameters.clone();
            cx.notify();
        });

        cx.update_entity(&self.current_model.quantization, |current_quant, cx| {
            *current_quant = quantization.clone();
            cx.notify();
        });

        self.save_model_selection(
            cx,
            "current",
            Some(provider_id),
            Some(provider_name),
            Some(model),
            parameters,
            quantization,
        );
    }

    pub fn get_provider(&self, cx: &App, provider_id: &UniqueId) -> Option<Arc<Provider>> {
        self.providers.read(cx).get(provider_id).cloned()
    }

    pub fn new_provider(
        &mut self,
        cx: &mut App,
        kind: ProviderKind,
        name: impl Into<String>,
        url: impl Into<String>,
        api_key: Option<SecretString>,
        errors: Entity<VecDeque<String>>,
    ) -> UniqueId {
        let provider_id = UniqueId::new();
        let name = name.into();
        let url = url.into();

        let api_key_for_init = api_key
            .as_ref()
            .map(|k| k.clone())
            .unwrap_or_else(|| SecretString::from(String::new()));

        if let Some(api_key) = api_key {
            let secret_name = Self::construct_provider_api_key_name(&provider_id, &name);
            set_secret(cx, &secret_name, api_key.expose_secret()).detach();
        }

        let db = self.db().clone();
        let now = DbDateTime::now();
        let provider_id_clone = provider_id.clone();
        let kind_str = kind.as_str().to_string();
        let name_clone = name.clone();
        let url_clone = url.clone();

        cx.spawn(async move |cx: &mut AsyncApp| {
            if let Err(e) = db
                .mutate(
                    AstrumDb::PROVIDERS.insert(
                        ProviderRecord::build()
                            .id(provider_id_clone)
                            .kind(kind_str)
                            .name(name_clone)
                            .url(url_clone)
                            .created_at(now.clone())
                            .edited_at(now),
                    ),
                )
                .execute()
                .await
            {
                crate::utils::errors::push_error_async(
                    &errors,
                    cx,
                    format!("Failed to save provider: {e}"),
                );
            }
        })
        .detach();

        let http_client = GpuiHttpWrapper::new(cx.http_client());
        self.init_provider(cx, &provider_id, &kind, name, url, api_key_for_init, http_client);

        provider_id
    }

    pub fn get_current_model(&'a self, cx: &'a App) -> Option<&'a String> {
        self.current_model.model.read(cx).as_ref()
    }

    pub fn clear_current_selection(&mut self, cx: &mut App) {
        cx.update_entity(&self.current_model.provider_id, |provider_id, cx| {
            *provider_id = None;
            cx.notify();
        });
        cx.update_entity(&self.current_model.provider_name, |provider_name, cx| {
            *provider_name = None;
            cx.notify();
        });
        cx.update_entity(&self.current_model.model, |model, cx| {
            *model = None;
            cx.notify();
        });
        cx.update_entity(&self.current_model.parameters, |params, cx| {
            *params = None;
            cx.notify();
        });
        cx.update_entity(&self.current_model.quantization, |quant, cx| {
            *quant = None;
            cx.notify();
        });
        self.save_model_selection(cx, "current", None, None, None, None, None);
    }

    pub fn clear_chat_titles_selection(&mut self, cx: &mut App) {
        cx.update_entity(&self.chat_titles_model.provider_id, |provider_id, cx| {
            *provider_id = None;
            cx.notify();
        });
        cx.update_entity(
            &self.chat_titles_model.provider_name,
            |provider_name, cx| {
                *provider_name = None;
                cx.notify();
            },
        );
        cx.update_entity(&self.chat_titles_model.model, |model, cx| {
            *model = None;
            cx.notify();
        });
        cx.update_entity(&self.chat_titles_model.parameters, |params, cx| {
            *params = None;
            cx.notify();
        });
        cx.update_entity(&self.chat_titles_model.quantization, |quant, cx| {
            *quant = None;
            cx.notify();
        });
        self.save_model_selection(cx, "chat_titles", None, None, None, None, None);
    }

    pub fn get_chat_titles_provider<'b>(&'b self, cx: &'b App) -> Option<&'b Arc<Provider>> {
        self.chat_titles_model
            .provider_id
            .read(cx)
            .as_ref()
            .and_then(|id| self.providers.read(cx).get(id))
    }

    pub fn set_chat_titles_selection(
        &mut self,
        cx: &mut App,
        provider_id: UniqueId,
        provider_name: impl Into<String>,
        model: impl Into<String>,
        parameters: Option<String>,
        quantization: Option<String>,
    ) {
        let provider_name = provider_name.into();
        let model = model.into();
        cx.update_entity(
            &self.chat_titles_model.provider_id,
            |current_provider_id, cx| {
                *current_provider_id = Some(provider_id.clone());
                cx.notify();
            },
        );
        cx.update_entity(
            &self.chat_titles_model.provider_name,
            |current_provider_name, cx| {
                *current_provider_name = Some(provider_name.clone());
                cx.notify();
            },
        );
        cx.update_entity(&self.chat_titles_model.model, |current_model, cx| {
            *current_model = Some(model.clone());
            cx.notify();
        });
        cx.update_entity(&self.chat_titles_model.parameters, |current_params, cx| {
            *current_params = parameters.clone();
            cx.notify();
        });
        cx.update_entity(&self.chat_titles_model.quantization, |current_quant, cx| {
            *current_quant = quantization.clone();
            cx.notify();
        });
        self.save_model_selection(
            cx,
            "chat_titles",
            Some(provider_id),
            Some(provider_name),
            Some(model),
            parameters,
            quantization,
        );
    }

    pub fn get_chat_titles_model(&'a self, cx: &'a App) -> Option<&'a String> {
        self.chat_titles_model.model.read(cx).as_ref()
    }

    fn save_model_selection(
        &self,
        cx: &mut App,
        key: &str,
        provider_id: Option<UniqueId>,
        provider_name: Option<String>,
        model: Option<String>,
        parameters: Option<String>,
        quantization: Option<String>,
    ) {
        let db = self.db().clone();
        let key = key.to_string();
        cx.spawn(async move |_cx| {
            // Use delete + insert since notitia doesn't have upsert
            if let Err(e) = db
                .mutate(
                    AstrumDb::MODEL_SELECTIONS
                        .delete()
                        .filter(ModelSelectionRecord::KEY.eq(key.clone())),
                )
                .execute()
                .await
            {
                error!("Failed to delete model selection '{}': {e}", key);
                return;
            }

            if let Err(e) = db
                .mutate(
                    AstrumDb::MODEL_SELECTIONS.insert(
                        ModelSelectionRecord::build()
                            .key(key.clone())
                            .provider_id(provider_id)
                            .provider_name(provider_name)
                            .model(model)
                            .parameters(parameters)
                            .quantization(quantization),
                    ),
                )
                .execute()
                .await
            {
                error!("Failed to save model selection '{}': {e}", key);
            }
        })
        .detach();
    }

    /// Asynchronously load providers and model selections from the DB,
    /// then apply them on the main thread via entity update.
    pub async fn load_from_db(
        models: Entity<ModelsManager>,
        db: Notitia<AstrumDb, SqliteAdapter>,
        cx: &mut AsyncApp,
    ) {
        // Query providers
        let providers_result: Result<
            BTreeMap<OrderKey, (PrimaryKey<UniqueId>, String, String, Option<String>)>,
            _,
        > = db
            .query(
                AstrumDb::PROVIDERS
                    .select((
                        ProviderRecord::ID,
                        ProviderRecord::KIND,
                        ProviderRecord::NAME,
                        ProviderRecord::URL,
                    ))
                    .order_by(ProviderRecord::CREATED_AT, OrderDirection::Asc)
                    .fetch_all::<BTreeMap<_, _>>(),
            )
            .execute()
            .await;

        // Query model selections
        let selections_result: Result<
            Vec<(
                PrimaryKey<String>,
                Option<UniqueId>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            )>,
            _,
        > = db
            .query(
                AstrumDb::MODEL_SELECTIONS
                    .select((
                        ModelSelectionRecord::KEY,
                        ModelSelectionRecord::PROVIDER_ID,
                        ModelSelectionRecord::PROVIDER_NAME,
                        ModelSelectionRecord::MODEL,
                        ModelSelectionRecord::PARAMETERS,
                        ModelSelectionRecord::QUANTIZATION,
                    ))
                    .fetch_all::<Vec<_>>(),
            )
            .execute()
            .await;

        // Read credentials for each provider async
        let mut api_keys: std::collections::HashMap<UniqueId, SecretString> =
            std::collections::HashMap::new();
        if let Ok(ref rows) = providers_result {
            for (_order, (provider_id, _kind_str, name, _url)) in rows {
                let id: &UniqueId = &*provider_id;
                let secret_name = ModelsManager::construct_provider_api_key_name(id, name);
                let task = models.update(cx, |_models, cx| get_secret(cx, &secret_name));
                let key = task.await.ok().unwrap_or_else(|| SecretString::from(String::new()));
                api_keys.insert(id.clone(), key);
            }
        }

        // Apply results on the main thread
        models.update(cx, |models, cx| {
            if let Ok(rows) = providers_result {
                for (_order, (provider_id, kind_str, name, url)) in rows {
                    let kind = ProviderKind::from_str(&kind_str);
                    let http_client = GpuiHttpWrapper::new(cx.http_client());
                    let id: &UniqueId = &*provider_id;
                    let api_key = api_keys
                        .remove(id)
                        .unwrap_or_else(|| SecretString::from(String::new()));
                    models.init_provider(
                        cx,
                        &*provider_id,
                        &kind,
                        name,
                        url.unwrap_or_else(|| kind.default_url().to_string()),
                        api_key,
                        http_client,
                    );
                }
            }

            if let Ok(rows) = selections_result {
                for (key, provider_id, provider_name, model, parameters, quantization) in rows {
                    let provider_exists = provider_id
                        .as_ref()
                        .map(|id| models.providers.read(cx).get(id).is_some())
                        .unwrap_or(false);

                    if !provider_exists {
                        continue;
                    }

                    let pair = match key.as_str() {
                        "current" => &models.current_model,
                        "chat_titles" => &models.chat_titles_model,
                        _ => continue,
                    };

                    if let Some(id) = provider_id {
                        pair.provider_id.update(cx, |pid, cx| {
                            *pid = Some(id);
                            cx.notify();
                        });
                    }
                    if let Some(name) = provider_name {
                        pair.provider_name.update(cx, |pname, cx| {
                            *pname = Some(name);
                            cx.notify();
                        });
                    }
                    if let Some(m) = model {
                        pair.model.update(cx, |model, cx| {
                            *model = Some(m);
                            cx.notify();
                        });
                    }
                    if let Some(p) = parameters {
                        pair.parameters.update(cx, |params, cx| {
                            *params = Some(p);
                            cx.notify();
                        });
                    }
                    if let Some(q) = quantization {
                        pair.quantization.update(cx, |quant, cx| {
                            *quant = Some(q);
                            cx.notify();
                        });
                    }
                }
            }
        });
    }

    fn create_provider_client(
        kind: &ProviderKind,
        url: String,
        api_key: SecretString,
        http_client: GpuiHttpWrapper,
    ) -> Arc<dyn ProviderTrait> {
        match kind {
            ProviderKind::Ollama => Arc::new(OllamaProvider::new(http_client).url(url)),
            ProviderKind::OpenAi => {
                Arc::new(OpenAiProvider::new(http_client, api_key).url(url))
            }
            ProviderKind::Anthropic => {
                Arc::new(AnthropicProvider::new(http_client, api_key).url(url))
            }
        }
    }

    fn init_provider(
        &mut self,
        cx: &mut App,
        provider_id: &UniqueId,
        kind: &ProviderKind,
        name: String,
        url: String,
        api_key: SecretString,
        http_client: GpuiHttpWrapper,
    ) -> Option<()> {
        let inner = Self::create_provider_client(kind, url.clone(), api_key, http_client);

        let icon = kind.default_icon().to_string();
        let logo = kind.default_logo().to_string();

        self.providers.update(cx, |providers, cx| {
            let provider = Arc::new(Provider::new(cx, inner, name, url, icon, logo));
            providers.insert_front(provider_id.clone(), provider);
            cx.notify();
        });

        Some(())
    }

    /// Reinitialize a provider's inner client with updated URL/API key.
    pub async fn reinit_provider(
        models: Entity<ModelsManager>,
        provider_id: UniqueId,
        cx: &mut AsyncApp,
    ) {
        // Read provider info from the main thread
        let Some((db, name, url)) = models.update(cx, |models, cx| {
            let provider = models.providers.read(cx).get(&provider_id).cloned()?;
            let db = models.db().clone();
            let name = provider.name.read(cx).to_string();
            let url = provider.url.read(cx).to_string();
            Some((db, name, url))
        }) else {
            return;
        };

        // Query kind from DB async
        let kind_result: Result<String, _> = db
            .query(
                AstrumDb::PROVIDERS
                    .select(ProviderRecord::KIND)
                    .filter(ProviderRecord::ID.eq(provider_id.clone()))
                    .fetch_one(),
            )
            .execute()
            .await;

        let Ok(kind_str) = kind_result else {
            return;
        };
        let kind = ProviderKind::from_str(&kind_str);

        // Read credential async
        let secret_name = Self::construct_provider_api_key_name(&provider_id, &name);
        let api_key = models.update(cx, |_models, cx| {
            get_secret(cx, &secret_name)
        }).await.ok().unwrap_or_else(|| SecretString::from(String::new()));

        // Apply on main thread
        models.update(cx, |models, cx| {
            let http_client = GpuiHttpWrapper::new(cx.http_client());
            let inner = Self::create_provider_client(&kind, url.clone(), api_key, http_client);

            let icon = kind.default_icon().to_string();
            let logo = kind.default_logo().to_string();

            models.providers.update(cx, |providers, cx| {
                let new_provider = Arc::new(Provider::new(cx, inner, name, url, icon, logo));
                providers.insert(provider_id.clone(), new_provider);
                cx.notify();
            });
        });
    }

    fn construct_provider_api_key_name(provider_id: &UniqueId, name: &str) -> String {
        format!("chat.astrum.astrum:provider:{}:{}", name, provider_id)
    }

    pub fn get_provider_api_key(
        &self,
        cx: &App,
        provider_id: &UniqueId,
    ) -> Task<Option<String>> {
        let Some(provider) = self.providers.read(cx).get(provider_id).cloned() else {
            return Task::ready(None);
        };

        let secret_name =
            Self::construct_provider_api_key_name(provider_id, &provider.name.read(cx));

        let task = get_secret(cx, &secret_name);
        cx.foreground_executor().spawn(async move {
            task.await.ok().map(|s| s.expose_secret().to_string())
        })
    }

    pub fn edit_provider_api_key(
        &mut self,
        cx: &mut App,
        provider_id: UniqueId,
        api_key: Option<String>,
    ) {
        let Some(provider) = self.providers.read(cx).get(&provider_id).cloned() else {
            return;
        };

        let secret_name =
            Self::construct_provider_api_key_name(&provider_id, &provider.name.read(cx));

        match api_key {
            Some(api_key) if !api_key.is_empty() => {
                set_secret(cx, &secret_name, &api_key).detach();
            }
            _ => {
                remove_secret(cx, &secret_name).detach();
            }
        }
    }

    pub fn edit_provider_url(&mut self, cx: &mut App, provider_id: UniqueId, url: String) {
        let db = self.db().clone();
        let provider_id_clone = provider_id.clone();
        let url_clone = url.clone();
        cx.spawn(async move |_cx| {
            if let Err(e) = db
                .mutate(
                    AstrumDb::PROVIDERS
                        .update(
                            ProviderRecord::build()
                                .url(url_clone)
                                .edited_at(DbDateTime::now()),
                        )
                        .filter(ProviderRecord::ID.eq(provider_id_clone)),
                )
                .execute()
                .await
            {
                error!("Failed to update provider URL: {e}");
            }
        })
        .detach();

        self.providers.update(cx, |providers, cx| {
            if let Some(provider) = providers.get(&provider_id) {
                provider.url.update(cx, |provider_url, cx| {
                    *provider_url = url.into();
                    cx.notify();
                });
            }
        });
    }

    pub fn delete_provider(
        &mut self,
        cx: &mut App,
        provider_id: UniqueId,
        errors: Entity<VecDeque<String>>,
    ) {
        let provider = self.providers.read(cx).get(&provider_id).cloned();

        let db = self.db().clone();
        let provider_id_clone = provider_id.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            if let Err(e) = db
                .mutate(
                    AstrumDb::PROVIDERS
                        .delete()
                        .filter(ProviderRecord::ID.eq(provider_id_clone)),
                )
                .execute()
                .await
            {
                crate::utils::errors::push_error_async(
                    &errors,
                    cx,
                    format!("Failed to delete provider: {e}"),
                );
            }
        })
        .detach();

        if let Some(provider) = provider {
            let secret_name =
                Self::construct_provider_api_key_name(&provider_id, &provider.name.read(cx));
            remove_secret(cx, &secret_name).detach();
        }

        self.models_cache.update(cx, |cache, _| {
            cache.delete_models_for_provider(&provider_id);
        });

        self.providers.update(cx, |providers, cx| {
            providers.remove(&provider_id);
            cx.notify();
        });
    }
}

#[derive(Assoc)]
#[func(pub fn as_str(&self) -> &'static str)]
#[func(pub fn default_name(&self) -> SharedString)]
#[func(pub fn default_url(&self) -> SharedString)]
#[func(pub fn default_icon(&self) -> SharedString)]
#[func(pub fn default_logo(&self) -> SharedString)]
pub enum ProviderKind {
    #[assoc(as_str = "ollama")]
    #[assoc(default_name = "Ollama".into())]
    #[assoc(default_url = "http://localhost:11434".into())]
    #[assoc(default_icon = AstrumProviderIconKind::Ollama.into())]
    #[assoc(default_logo = AstrumProviderLogoKind::Ollama.into())]
    Ollama,

    #[assoc(as_str = "anthropic")]
    #[assoc(default_name = "Anthropic".into())]
    #[assoc(default_url = "https://api.anthropic.com".into())]
    #[assoc(default_icon = AstrumProviderIconKind::Anthropic.into())]
    #[assoc(default_logo = AstrumProviderLogoKind::Anthropic.into())]
    Anthropic,

    #[assoc(as_str = "openai")]
    #[assoc(default_name = "OpenAI".into())]
    #[assoc(default_url = "https://api.openai.com".into())]
    #[assoc(default_icon = AstrumProviderIconKind::OpenAi.into())]
    #[assoc(default_logo = AstrumProviderLogoKind::OpenAi.into())]
    OpenAi,
}

impl ProviderKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "ollama" => Self::Ollama,
            "anthropic" => Self::Anthropic,
            "openai" => Self::OpenAi,
            _ => Self::Ollama,
        }
    }
}

#[derive(Clone)]
pub struct Provider {
    pub inner: Arc<dyn ProviderTrait>,
    pub name: Entity<SharedString>,
    pub url: Entity<SharedString>,
    pub icon: Entity<SharedString>,
    pub logo: Entity<SharedString>,
}

impl Provider {
    fn new(
        cx: &mut App,
        inner: Arc<dyn ProviderTrait>,
        name: impl Into<SharedString>,
        url: impl Into<SharedString>,
        icon: impl Into<SharedString>,
        logo: impl Into<SharedString>,
    ) -> Self {
        Self {
            inner,
            name: cx.new(|_cx| name.into()),
            url: cx.new(|_cx| url.into()),
            icon: cx.new(|_cx| icon.into()),
            logo: cx.new(|_cx| logo.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use notitia::prelude::*;
    use notitia::{Notitia, PrimaryKey};
    use notitia_sqlite::SqliteAdapter;
    use schema::{AstrumDb, ModelSelectionRecord, UniqueId};

    async fn test_db() -> Notitia<AstrumDb, SqliteAdapter> {
        AstrumDb::connect::<SqliteAdapter>("sqlite::memory:")
            .await
            .expect("Failed to create in-memory test DB")
    }

    async fn upsert_selection(
        db: &Notitia<AstrumDb, SqliteAdapter>,
        key: &str,
        provider_id: Option<UniqueId>,
        provider_name: Option<String>,
        model: Option<String>,
    ) -> anyhow::Result<()> {
        db.mutate(
            AstrumDb::MODEL_SELECTIONS
                .delete()
                .filter(ModelSelectionRecord::KEY.eq(key.to_string())),
        )
        .execute()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        db.mutate(
            AstrumDb::MODEL_SELECTIONS.insert(
                ModelSelectionRecord::build()
                    .key(key.to_string())
                    .provider_id(provider_id)
                    .provider_name(provider_name)
                    .model(model)
                    .parameters(None::<String>)
                    .quantization(None::<String>),
            ),
        )
        .execute()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(())
    }

    async fn query_selection(
        db: &Notitia<AstrumDb, SqliteAdapter>,
        key: &str,
    ) -> Option<(String, Option<String>)> {
        let result: Result<(PrimaryKey<String>, Option<String>), _> = db
            .query(
                AstrumDb::MODEL_SELECTIONS
                    .select((ModelSelectionRecord::KEY, ModelSelectionRecord::MODEL))
                    .filter(ModelSelectionRecord::KEY.eq(key.to_string()))
                    .fetch_one(),
            )
            .execute()
            .await;
        result.ok().map(|(pk, model): (PrimaryKey<String>, Option<String>)| (pk.into_inner(), model))
    }

    #[test]
    fn test_insert_new_model_selection() {
        smol::block_on(async {
            let db = test_db().await;
            upsert_selection(&db, "current", Some(UniqueId::new()), Some("OpenAI".into()), Some("gpt-4".into()))
                .await
                .unwrap();

            let result = query_selection(&db, "current").await;
            assert!(result.is_some());
            let (key, model) = result.unwrap();
            assert_eq!(key, "current");
            assert_eq!(model, Some("gpt-4".to_string()));
        });
    }

    #[test]
    fn test_upsert_overwrites_existing_selection() {
        smol::block_on(async {
            let db = test_db().await;
            let pid = UniqueId::new();

            upsert_selection(&db, "current", Some(pid.clone()), Some("OpenAI".into()), Some("gpt-4".into()))
                .await
                .unwrap();

            upsert_selection(&db, "current", Some(pid), Some("OpenAI".into()), Some("gpt-4o".into()))
                .await
                .unwrap();

            let (_, model) = query_selection(&db, "current").await.unwrap();
            assert_eq!(model, Some("gpt-4o".to_string()));
        });
    }

    #[test]
    fn test_different_keys_are_independent() {
        smol::block_on(async {
            let db = test_db().await;

            upsert_selection(&db, "current", None, None, Some("gpt-4".into()))
                .await
                .unwrap();
            upsert_selection(&db, "chat_titles", None, None, Some("gpt-3.5".into()))
                .await
                .unwrap();

            let (_, m1) = query_selection(&db, "current").await.unwrap();
            let (_, m2) = query_selection(&db, "chat_titles").await.unwrap();
            assert_eq!(m1, Some("gpt-4".to_string()));
            assert_eq!(m2, Some("gpt-3.5".to_string()));
        });
    }

    #[test]
    fn test_delete_nonexistent_key_does_not_error() {
        smol::block_on(async {
            let db = test_db().await;
            let result = upsert_selection(&db, "nonexistent", None, None, Some("model".into())).await;
            assert!(result.is_ok());
        });
    }
}
