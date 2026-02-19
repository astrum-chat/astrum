use std::sync::Arc;

use anyml::{
    AnthropicProvider, OllamaProvider, OpenAiProvider,
    providers::{chat::ChatProvider, list_models::ListModelsProvider},
};
use enum_assoc::Assoc;
use gpui::{App, AppContext, Entity, SharedString};
use notitia::prelude::*;
use notitia::{Notitia, PrimaryKey};
use notitia_sqlite::SqliteAdapter;
use secrecy::{ExposeSecret, SecretString};

pub trait ProviderTrait: ChatProvider + ListModelsProvider {}
impl<T: ChatProvider + ListModelsProvider> ProviderTrait for T {}

use crate::{
    anyhttp_gpui::GpuiHttpWrapper,
    assets::AstrumLogoKind,
    blocks::models_menu::ModelsCache,
    managers::UniqueId,
    schema::{AstrumDb, DbDateTime, ModelSelectionRecord, ProviderRecord},
    secrets::{get_secret, remove_secret, set_secret},
    utils::FrontInsertMap,
};

pub struct ProviderModelPair {
    pub provider_id: Entity<Option<UniqueId>>,
    pub provider_name: Entity<Option<String>>,
    pub model: Entity<Option<String>>,
}

impl ProviderModelPair {
    pub fn read_selection(&self, cx: &App) -> (Option<UniqueId>, Option<String>, Option<String>) {
        (
            self.provider_id.read(cx).clone(),
            self.provider_name.read(cx).clone(),
            self.model.read(cx).clone(),
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
            },
            chat_titles_model: ProviderModelPair {
                provider_id: cx.new(|_cx| None),
                provider_name: cx.new(|_cx| None),
                model: cx.new(|_cx| None),
            },
            models_cache: cx.new(|_cx| ModelsCache::new()),
        }
    }

    pub fn init(&mut self, cx: &mut App, db: Notitia<AstrumDb, SqliteAdapter>) {
        self.db = Some(db);

        // Load providers synchronously at startup by blocking on the async query.
        // This is acceptable during init since nothing else is happening.
        self.load_providers_from_db_sync(cx);
        self.load_model_selections_from_db_sync(cx);
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
        self.save_model_selection(
            cx,
            "current",
            Some(provider_id),
            Some(provider_name),
            Some(model),
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
        icon: Option<String>,
        api_key: Option<SecretString>,
    ) -> UniqueId {
        let provider_id = UniqueId::new();
        let name = name.into();
        let url = url.into();

        if let Some(api_key) = api_key {
            let secret_name = &Self::construct_provider_api_key_name(&provider_id, &name);
            let _ = set_secret(secret_name, api_key.expose_secret());
        }

        let db = self.db().clone();
        let now = DbDateTime::now();
        let provider_id_clone = provider_id.clone();
        let kind_str = kind.as_str().to_string();
        let name_clone = name.clone();
        let url_clone = url.clone();
        let icon_clone = icon.clone();
        cx.spawn(async move |_cx| {
            db.mutate(
                AstrumDb::PROVIDERS.insert(
                    ProviderRecord::build()
                        .id(provider_id_clone)
                        .kind(kind_str)
                        .name(name_clone)
                        .url(url_clone)
                        .icon(icon_clone)
                        .created_at(now.clone())
                        .edited_at(now),
                ),
            )
            .execute()
            .await
            .unwrap();
        })
        .detach();

        let http_client = GpuiHttpWrapper::new(cx.http_client());
        self.init_provider(
            cx,
            &provider_id,
            &kind,
            name,
            url,
            icon.unwrap_or_else(|| kind.default_icon().to_string()),
            http_client,
        );

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
        self.save_model_selection(cx, "current", None, None, None);
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
        self.save_model_selection(cx, "chat_titles", None, None, None);
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
        self.save_model_selection(
            cx,
            "chat_titles",
            Some(provider_id),
            Some(provider_name),
            Some(model),
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
    ) {
        let db = self.db().clone();
        let key = key.to_string();
        cx.spawn(async move |_cx| {
            // Use delete + insert since notitia doesn't have upsert
            db.mutate(
                AstrumDb::MODEL_SELECTIONS
                    .delete()
                    .filter(ModelSelectionRecord::KEY.eq(key.clone())),
            )
            .execute()
            .await
            .ok();

            db.mutate(
                AstrumDb::MODEL_SELECTIONS.insert(
                    ModelSelectionRecord::build()
                        .key(key)
                        .provider_id(provider_id)
                        .provider_name(provider_name)
                        .model(model),
                ),
            )
            .execute()
            .await
            .ok();
        })
        .detach();
    }

    fn load_providers_from_db_sync(&mut self, cx: &mut App) {
        // Use smol::block_on to load providers synchronously at init time.
        // This is acceptable because init() is called once at startup.
        let db = self.db().clone();
        let result: Result<
            BTreeMap<
                OrderKey,
                (
                    PrimaryKey<UniqueId>,
                    String,
                    String,
                    Option<String>,
                    Option<String>,
                ),
            >,
            _,
        > = smol::block_on(async {
            db.query(
                AstrumDb::PROVIDERS
                    .select((
                        ProviderRecord::ID,
                        ProviderRecord::KIND,
                        ProviderRecord::NAME,
                        ProviderRecord::URL,
                        ProviderRecord::ICON,
                    ))
                    .order_by(ProviderRecord::CREATED_AT, OrderDirection::Asc)
                    .fetch_all::<BTreeMap<_, _>>(),
            )
            .execute()
            .await
        });

        if let Ok(rows) = result {
            for (_order, (provider_id, kind_str, name, url, icon)) in rows {
                let kind = ProviderKind::from_str(&kind_str);
                let http_client = GpuiHttpWrapper::new(cx.http_client());
                self.init_provider(
                    cx,
                    &*provider_id,
                    &kind,
                    name,
                    url.unwrap_or_else(|| kind.default_url().to_string()),
                    icon.unwrap_or_else(|| kind.default_icon().to_string()),
                    http_client,
                );
            }
        }
    }

    fn load_model_selections_from_db_sync(&mut self, cx: &mut App) {
        let db = self.db().clone();
        let result: Result<
            Vec<(
                PrimaryKey<String>,
                Option<UniqueId>,
                Option<String>,
                Option<String>,
            )>,
            _,
        > = smol::block_on(async {
            db.query(
                AstrumDb::MODEL_SELECTIONS
                    .select((
                        ModelSelectionRecord::KEY,
                        ModelSelectionRecord::PROVIDER_ID,
                        ModelSelectionRecord::PROVIDER_NAME,
                        ModelSelectionRecord::MODEL,
                    ))
                    .fetch_all::<Vec<_>>(),
            )
            .execute()
            .await
        });

        let Ok(rows) = result else { return };

        for (key, provider_id, provider_name, model) in rows {
            let provider_exists = provider_id
                .as_ref()
                .map(|id| self.providers.read(cx).get(id).is_some())
                .unwrap_or(false);

            if !provider_exists {
                continue;
            }

            match key.as_str() {
                "current" => {
                    if let Some(id) = provider_id {
                        self.current_model.provider_id.update(cx, |pid, cx| {
                            *pid = Some(id);
                            cx.notify();
                        });
                    }
                    if let Some(name) = provider_name {
                        self.current_model.provider_name.update(cx, |pname, cx| {
                            *pname = Some(name);
                            cx.notify();
                        });
                    }
                    if let Some(m) = model {
                        self.current_model.model.update(cx, |model, cx| {
                            *model = Some(m);
                            cx.notify();
                        });
                    }
                }
                "chat_titles" => {
                    if let Some(id) = provider_id {
                        self.chat_titles_model.provider_id.update(cx, |pid, cx| {
                            *pid = Some(id);
                            cx.notify();
                        });
                    }
                    if let Some(name) = provider_name {
                        self.chat_titles_model
                            .provider_name
                            .update(cx, |pname, cx| {
                                *pname = Some(name);
                                cx.notify();
                            });
                    }
                    if let Some(m) = model {
                        self.chat_titles_model.model.update(cx, |model, cx| {
                            *model = Some(m);
                            cx.notify();
                        });
                    }
                }
                _ => {}
            }
        }
    }

    fn create_provider_client(
        kind: &ProviderKind,
        provider_id: &UniqueId,
        name: &str,
        url: String,
        http_client: GpuiHttpWrapper,
    ) -> Arc<dyn ProviderTrait> {
        match kind {
            ProviderKind::Ollama => Arc::new(OllamaProvider::new(http_client).url(url)),
            ProviderKind::OpenAi => {
                let secret_name = Self::construct_provider_api_key_name(provider_id, name);
                let api_key = get_secret(&secret_name).unwrap_or_default();
                Arc::new(OpenAiProvider::new(http_client, api_key).url(url))
            }
            ProviderKind::Anthropic => {
                let secret_name = Self::construct_provider_api_key_name(provider_id, name);
                let api_key = get_secret(&secret_name).unwrap_or_default();
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
        icon: String,
        http_client: GpuiHttpWrapper,
    ) -> Option<()> {
        let inner =
            Self::create_provider_client(kind, provider_id, &name, url.clone(), http_client);

        self.providers.update(cx, |providers, cx| {
            let provider = Arc::new(Provider::new(cx, inner, name, url, icon));
            providers.insert_front(provider_id.clone(), provider);
            cx.notify();
        });

        Some(())
    }

    /// Reinitialize a provider's inner client with updated URL/API key.
    pub fn reinit_provider(&mut self, cx: &mut App, provider_id: &UniqueId) {
        let Some(provider) = self.providers.read(cx).get(provider_id).cloned() else {
            return;
        };

        // kind is not stored in Provider, so query the DB.
        let db = self.db().clone();
        let provider_id_clone = provider_id.clone();
        let kind_result: Result<String, _> = smol::block_on(async {
            db.query(
                AstrumDb::PROVIDERS
                    .select(ProviderRecord::KIND)
                    .filter(ProviderRecord::ID.eq(provider_id_clone))
                    .fetch_one(),
            )
            .execute()
            .await
        });

        let Ok(kind_str) = kind_result else {
            return;
        };
        let kind = ProviderKind::from_str(&kind_str);

        let name = provider.name.read(cx).to_string();
        let url = provider.url.read(cx).to_string();
        let icon = provider.icon.read(cx).to_string();

        let http_client = GpuiHttpWrapper::new(cx.http_client());
        let inner =
            Self::create_provider_client(&kind, provider_id, &name, url.clone(), http_client);

        self.providers.update(cx, |providers, cx| {
            let new_provider = Arc::new(Provider::new(cx, inner, name, url, icon));
            providers.insert(provider_id.clone(), new_provider);
            cx.notify();
        });
    }

    fn construct_provider_api_key_name(provider_id: &UniqueId, name: &str) -> String {
        format!("chat.astrum.astrum:provider:{}:{}", name, provider_id)
    }

    pub fn get_provider_api_key(&self, cx: &App, provider_id: &UniqueId) -> Option<String> {
        let provider = self.providers.read(cx).get(provider_id).cloned()?;

        let secret_name =
            Self::construct_provider_api_key_name(provider_id, &provider.name.read(cx));

        get_secret(&secret_name)
            .ok()
            .map(|s| s.expose_secret().to_string())
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
                let _ = set_secret(&secret_name, &api_key).unwrap();
            }
            _ => {
                let _ = remove_secret(&secret_name);
            }
        }
    }

    pub fn edit_provider_url(&mut self, cx: &mut App, provider_id: UniqueId, url: String) {
        let db = self.db().clone();
        let provider_id_clone = provider_id.clone();
        let url_clone = url.clone();
        cx.spawn(async move |_cx| {
            db.mutate(
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
            .unwrap();
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

    pub fn delete_provider(&mut self, cx: &mut App, provider_id: UniqueId) {
        let provider = self.providers.read(cx).get(&provider_id).cloned();

        let db = self.db().clone();
        let provider_id_clone = provider_id.clone();
        cx.spawn(async move |_cx| {
            db.mutate(
                AstrumDb::PROVIDERS
                    .delete()
                    .filter(ProviderRecord::ID.eq(provider_id_clone)),
            )
            .execute()
            .await
            .unwrap();
        })
        .detach();

        if let Some(provider) = provider {
            let secret_name =
                Self::construct_provider_api_key_name(&provider_id, &provider.name.read(cx));
            let _ = remove_secret(&secret_name);
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
pub enum ProviderKind {
    #[assoc(as_str = "ollama")]
    #[assoc(default_name = "Ollama".into())]
    #[assoc(default_url = "http://localhost:11434".into())]
    #[assoc(default_icon = AstrumLogoKind::Ollama.into())]
    Ollama,

    #[assoc(as_str = "anthropic")]
    #[assoc(default_name = "Anthropic".into())]
    #[assoc(default_url = "https://api.anthropic.com".into())]
    #[assoc(default_icon = AstrumLogoKind::Anthropic.into())]
    Anthropic,

    #[assoc(as_str = "openai")]
    #[assoc(default_name = "OpenAI".into())]
    #[assoc(default_url = "https://api.openai.com".into())]
    #[assoc(default_icon = AstrumLogoKind::OpenAi.into())]
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
}

impl Provider {
    fn new(
        cx: &mut App,
        inner: Arc<dyn ProviderTrait>,
        name: impl Into<SharedString>,
        url: impl Into<SharedString>,
        icon: impl Into<SharedString>,
    ) -> Self {
        Self {
            inner,
            name: cx.new(|_cx| name.into()),
            url: cx.new(|_cx| url.into()),
            icon: cx.new(|_cx| icon.into()),
        }
    }
}
