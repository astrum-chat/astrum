use std::borrow::Cow;

use enum_assoc::Assoc;
use gpui::{Result, SharedString};
use gpui_tesserae::AssetProvider;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets/app"]
pub struct AstrumAssets;

impl AssetProvider for AstrumAssets {
    fn get(&self, path: &str) -> Option<Cow<'static, [u8]>> {
        <Self as RustEmbed>::get(path).map(|f| f.data)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(AstrumAssets::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

#[derive(Assoc)]
#[func(pub const fn path(&self) -> &'static str)]
pub enum AstrumIconKind {
    #[assoc(path = "icons/logo.svg")]
    Logo,

    #[assoc(path = "icons/plus.svg")]
    Plus,

    #[assoc(path = "icons/thick_plus.svg")]
    ThickPlus,

    #[assoc(path = "icons/search.svg")]
    Search,

    #[assoc(path = "icons/send.svg")]
    Send,

    #[assoc(path = "icons/think.svg")]
    Think,

    #[assoc(path = "icons/chat.svg")]
    Chat,

    #[assoc(path = "icons/web.svg")]
    Web,

    #[assoc(path = "icons/settings.svg")]
    Settings,

    #[assoc(path = "icons/key.svg")]
    Key,

    #[assoc(path = "icons/title.svg")]
    Title,

    #[assoc(path = "icons/trash.svg")]
    Trash,
}

impl Into<SharedString> for AstrumIconKind {
    fn into(self) -> SharedString {
        self.path().into()
    }
}

impl Into<SharedString> for &AstrumIconKind {
    fn into(self) -> SharedString {
        self.path().into()
    }
}

#[derive(Assoc)]
#[func(pub const fn path(&self) -> &'static str)]
pub enum AstrumLogoKind {
    #[assoc(path = "logos/providers/anthropic.svg")]
    Anthropic,

    #[assoc(path = "logos/providers/gemini.svg")]
    Gemini,

    #[assoc(path = "logos/providers/ollama.svg")]
    Ollama,

    #[assoc(path = "logos/providers/openai.svg")]
    OpenAi,

    #[assoc(path = "logos/providers/xai.svg")]
    Xai,
}

impl Into<SharedString> for AstrumLogoKind {
    fn into(self) -> SharedString {
        self.path().into()
    }
}

impl Into<SharedString> for &AstrumLogoKind {
    fn into(self) -> SharedString {
        self.path().into()
    }
}
