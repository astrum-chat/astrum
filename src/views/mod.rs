use gpui::{Pixels, px};

mod backdrop;
pub use backdrop::*;

mod chat;
pub use chat::*;

mod model_discovery;
pub use model_discovery::*;

mod settings;
pub use settings::*;

#[cfg(all(target_os = "macos", HAS_LIQUID_GLASS_WINDOW))]
pub const MACOS_TITLEBAR_PADDING: Pixels = px(33.);

#[cfg(all(target_os = "macos", not(HAS_LIQUID_GLASS_WINDOW)))]
pub const MACOS_TITLEBAR_PADDING: Pixels = px(34.);

pub const FULLSCREEN_PADDING: Pixels = px(10.);
