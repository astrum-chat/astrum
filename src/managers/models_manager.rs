use std::sync::Arc;

use anyml::{
    AnthropicProvider, OllamaProvider, OpenAiProvider,
    providers::{chat::ChatProvider, list_models::ListModelsProvider},
};
use chrono::Utc;
use enum_assoc::Assoc;
use gpui::{App, AppContext, Entity, SharedString};
use rusqlite::{
    ToSql,
    types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef},
};
use secrecy::{ExposeSecret, SecretString};

pub trait ProviderTrait: ChatProvider + ListModelsProvider {}
impl<T: ChatProvider + ListModelsProvider> ProviderTrait for T {}

use crate::{
    anyhttp_gpui::GpuiHttpWrapper,
    assets::AstrumLogoKind,
    blocks::models_menu::ModelsCache,
    managers::{DbError, UniqueId},
    secrets::{get_secret, remove_secret, set_secret},
    utils::FrontInsertMap,
};

pub struct ProviderModelPair {
    pub provider_id: Entity<Option<UniqueId>>,
    pub provider_name: Entity<Option<String>>,
    pub model: Entity<Option<String>>,
}

pub struct ModelsManager {
    db_connection: Option<Arc<rusqlite::Connection>>,
    pub providers: Entity<FrontInsertMap<UniqueId, Arc<Provider>>>,
    pub current_model: ProviderModelPair,
    pub chat_titles_model: ProviderModelPair,
    /// Cache for provider models
    pub models_cache: Entity<ModelsCache>,
}

impl<'a> ModelsManager {
    pub fn new(cx: &mut App) -> Self {
        Self {
            db_connection: None,
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

    pub fn init(&mut self, cx: &mut App, db_connection: Arc<rusqlite::Connection>) {
        db_connection
            .execute_batch(
                "
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS providers (
                    id         TEXT PRIMARY KEY,
                    kind       TEXT NOT NULL
                        CHECK (kind IN ('ollama', 'anthropic', 'openai')),
                    name       TEXT NOT NULL,
                    url        TEXT NOT NULL,
                    icon       TEXT,
                    created_at DATETIME NOT NULL,
                    edited_at  DATETIME NOT NULL
                );

                CREATE TABLE IF NOT EXISTS model_selections (
                    key           TEXT PRIMARY KEY CHECK (key IN ('current', 'chat_titles')),
                    provider_id   TEXT,
                    provider_name TEXT,
                    model         TEXT
                );
                ",
            )
            .unwrap();

        let _ = self
            .load_providers_from_db(cx, db_connection.clone())
            .unwrap();

        self.load_model_selections_from_db(cx, &db_connection);

        self.db_connection = Some(db_connection);
    }

    pub fn get_current_provider<'b>(&'b self, cx: &'b App) -> Option<&'b Arc<Provider>> {
        self.current_model
            .provider_id
            .read(cx)
            .as_ref()
            .map(|this| self.providers.read(cx).get(this))
            .flatten()
    }

    pub fn set_current_provider(
        &mut self,
        cx: &mut App,
        provider_id: UniqueId,
        provider_name: impl Into<String>,
    ) {
        let provider_name = provider_name.into();
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
        self.save_model_selection(
            "current",
            Some(&provider_id),
            Some(&provider_name),
            self.current_model.model.read(cx).as_deref(),
        );
    }

    pub fn get_provider(
        &mut self,
        cx: &mut App,
        provider_id: &UniqueId,
    ) -> Result<Arc<Provider>, DbError> {
        let provider_id = provider_id.as_ref();

        if let Some(provider) = self.providers.read(cx).get(provider_id) {
            return Ok(provider.clone());
        }

        let db = self
            .db_connection
            .as_ref()
            .ok_or_else(|| DbError::MissingData("db_connection"))?
            .clone();

        let mut stmt = db
            .prepare(
                r#"
                    SELECT
                        kind,
                        name,
                        url,
                        icon
                    FROM providers
                    WHERE id = ?1
                    "#,
            )
            .map_err(|err| DbError::SqliteError(err))?;

        let (kind, name, url, icon) = stmt
            .query_row([provider_id.to_string()], |row| {
                Ok((
                    row.get::<_, ProviderKind>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(|err| DbError::SqliteError(err))?;

        let http_client = GpuiHttpWrapper::new(cx.http_client());

        self.init_provider(
            cx,
            provider_id,
            &kind,
            name,
            url,
            icon.unwrap_or_else(|| kind.default_icon().to_string()),
            http_client,
        )
        .ok_or_else(|| DbError::MissingData("provider"))?;

        self.providers
            .read(cx)
            .get(provider_id)
            .cloned()
            .ok_or_else(|| DbError::MissingData("provider"))
    }

    pub fn new_provider(
        &mut self,
        cx: &mut App,
        kind: ProviderKind,
        name: impl Into<String>,
        url: impl Into<String>,
        icon: Option<String>,
        api_key: Option<SecretString>,
    ) -> Result<UniqueId, DbError> {
        let db_connection = self
            .db_connection
            .as_ref()
            .ok_or_else(|| DbError::MissingData("database connection"))?;

        let created_at = Utc::now().naive_utc();
        let provider_id = UniqueId::new();

        let name = name.into();
        let url = url.into();

        db_connection.execute(
            "INSERT INTO providers (id, kind, name, url, icon, created_at, edited_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            (&provider_id, &kind, &name, &url, &icon, &created_at),
        ).map_err(|err| DbError::SqliteError(err))?;

        let http_client = GpuiHttpWrapper::new(cx.http_client());

        if let Some(api_key) = api_key {
            let secret_name = &Self::construct_provider_api_key_name(&provider_id, &name);
            let _ = set_secret(secret_name, api_key.expose_secret());
        }

        self.init_provider(
            cx,
            &provider_id,
            &kind,
            name,
            url,
            kind.default_icon().to_string(),
            http_client,
        );

        Ok(provider_id)
    }

    pub fn get_current_model(&'a self, cx: &'a App) -> Option<&'a String> {
        self.current_model.model.read(cx).as_ref()
    }

    pub fn set_current_model(&mut self, cx: &mut App, model_name: impl Into<String>) {
        let model_name = model_name.into();
        cx.update_entity(&self.current_model.model, |model, cx| {
            *model = Some(model_name.clone());
            cx.notify();
        });
        self.save_model_selection(
            "current",
            self.current_model.provider_id.read(cx).as_ref(),
            self.current_model.provider_name.read(cx).as_deref(),
            Some(&model_name),
        );
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
        self.save_model_selection("current", None, None, None);
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
        self.save_model_selection("chat_titles", None, None, None);
    }

    pub fn get_chat_titles_provider<'b>(&'b self, cx: &'b App) -> Option<&'b Arc<Provider>> {
        self.chat_titles_model
            .provider_id
            .read(cx)
            .as_ref()
            .map(|this| self.providers.read(cx).get(this))
            .flatten()
    }

    pub fn set_chat_titles_provider(
        &mut self,
        cx: &mut App,
        provider_id: UniqueId,
        provider_name: impl Into<String>,
    ) {
        let provider_name = provider_name.into();
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
        self.save_model_selection(
            "chat_titles",
            Some(&provider_id),
            Some(&provider_name),
            self.chat_titles_model.model.read(cx).as_deref(),
        );
    }

    pub fn get_chat_titles_model(&'a self, cx: &'a App) -> Option<&'a String> {
        self.chat_titles_model.model.read(cx).as_ref()
    }

    pub fn set_chat_titles_model(&mut self, cx: &mut App, model_name: impl Into<String>) {
        let model_name = model_name.into();
        cx.update_entity(&self.chat_titles_model.model, |model, cx| {
            *model = Some(model_name.clone());
            cx.notify();
        });
        self.save_model_selection(
            "chat_titles",
            self.chat_titles_model.provider_id.read(cx).as_ref(),
            self.chat_titles_model.provider_name.read(cx).as_deref(),
            Some(&model_name),
        );
    }

    fn save_model_selection(
        &self,
        key: &str,
        provider_id: Option<&UniqueId>,
        provider_name: Option<&str>,
        model: Option<&str>,
    ) {
        if let Some(db) = &self.db_connection {
            let _ = db.execute(
                "INSERT OR REPLACE INTO model_selections (key, provider_id, provider_name, model) VALUES (?1, ?2, ?3, ?4)",
                (key, provider_id, provider_name, model),
            );
        }
    }

    fn load_model_selections_from_db(&mut self, cx: &mut App, db: &rusqlite::Connection) {
        let mut stmt = match db.prepare(
            "SELECT key, provider_id, provider_name, model FROM model_selections WHERE key IN ('current', 'chat_titles')",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return,
        };

        let rows = match stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        }) {
            Ok(rows) => rows,
            Err(_) => return,
        };

        for row_result in rows {
            let Ok((key, provider_id_str, provider_name, model)) = row_result else {
                continue;
            };

            let provider_id = provider_id_str.map(UniqueId::from_string);

            // Check if provider exists before loading
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

    fn load_providers_from_db(
        &mut self,
        cx: &mut App,
        db_connection: Arc<rusqlite::Connection>,
    ) -> Result<(), DbError> {
        let mut stmt = db_connection
            .prepare(
                r#"
            SELECT
                id,
                kind,
                name,
                url,
                icon
            FROM providers
            ORDER BY created_at
            "#,
            )
            .map_err(|err| DbError::SqliteError(err))?;

        let rows = stmt
            .query_map([], |row| {
                let provider_id = UniqueId::from_string(row.get::<_, String>(0)?);
                let kind = row.get::<_, ProviderKind>(1)?;
                let name = row.get::<_, String>(2)?;
                let url = row.get::<_, String>(3)?;
                let icon = row.get::<_, Option<String>>(4)?;

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

                Ok(())
            })
            .map_err(DbError::SqliteError)?;

        for row_result in rows {
            row_result.map_err(DbError::SqliteError)?;
        }

        Ok(())
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
        let inner: Arc<dyn ProviderTrait> = match kind {
            ProviderKind::Ollama => Arc::new(OllamaProvider::new(http_client).url(url.clone())),
            ProviderKind::OpenAi => {
                let provider_api_key =
                    get_secret(Self::construct_provider_api_key_name(&provider_id, &name))
                        .unwrap_or_default();

                Arc::new(OpenAiProvider::new(http_client, provider_api_key).url(url.clone()))
            }
            ProviderKind::Anthropic => {
                let provider_api_key =
                    get_secret(Self::construct_provider_api_key_name(&provider_id, &name))
                        .unwrap_or_default();

                Arc::new(AnthropicProvider::new(http_client, provider_api_key).url(url.clone()))
            }
        };

        self.providers.update(cx, |providers, cx| {
            let provider = Arc::new(Provider::new(cx, inner, name, url, icon));
            providers.insert_front(provider_id.clone(), provider);
            cx.notify();
        });

        Some(())
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
    ) -> Result<(), DbError> {
        let provider = self.get_provider(cx, &provider_id)?;

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

        Ok(())
    }

    pub fn edit_provider_url(
        &mut self,
        cx: &mut App,
        provider_id: UniqueId,
        url: String,
    ) -> Result<(), DbError> {
        let _provider = self.get_provider(cx, &provider_id)?;

        let db = self
            .db_connection
            .as_ref()
            .ok_or_else(|| DbError::MissingData("db_connection"))?;

        let edited_at = Utc::now().naive_utc();

        db.execute(
            r#"
                UPDATE providers
                SET url = ?1, edited_at = ?2
                WHERE id = ?3
                "#,
            (&url, &edited_at, &provider_id),
        )
        .map_err(DbError::SqliteError)?;

        self.providers.update(cx, |providers, cx| {
            if let Some(provider) = providers.get(&provider_id) {
                provider.url.update(cx, |provider_url, cx| {
                    *provider_url = url.into();
                    cx.notify();
                });
            }
        });

        Ok(())
    }

    pub fn delete_provider(&mut self, cx: &mut App, provider_id: UniqueId) -> Result<(), DbError> {
        let provider = self.get_provider(cx, &provider_id)?;

        let db = self
            .db_connection
            .as_ref()
            .ok_or_else(|| DbError::MissingData("db_connection"))?;

        db.execute("DELETE FROM providers WHERE id = ?1", [&provider_id])
            .map_err(DbError::SqliteError)?;

        let secret_name =
            Self::construct_provider_api_key_name(&provider_id, &provider.name.read(cx));
        let _ = remove_secret(&secret_name);

        // Delete cached models for this provider
        self.models_cache.update(cx, |cache, _| {
            cache.delete_models_for_provider(&provider_id);
        });

        self.providers.update(cx, |providers, cx| {
            providers.remove(&provider_id);
            cx.notify();
        });

        Ok(())
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

impl ToSql for ProviderKind {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(match self {
            Self::Ollama => "ollama",
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
        }))
    }
}

impl FromSql for ProviderKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "ollama" => Ok(Self::Ollama),
            "anthropic" => Ok(Self::Anthropic),
            "openai" => Ok(Self::OpenAi),
            other => Err(FromSqlError::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown ProviderKind: {other}"),
            )))),
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
