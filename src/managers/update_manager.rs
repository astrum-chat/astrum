use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::AsyncReadExt;
use gpui::{
    App, AppContext, Entity,
    http_client::{AsyncBody, HttpClient},
};
use semver::Version;

const CHECK_INTERVAL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
pub struct ReleaseInfo {
    pub version: String,
    pub html_url: String,
    pub found_at: Instant,
}

pub struct UpdateManager {
    pub available_update: Entity<Option<ReleaseInfo>>,
}

impl UpdateManager {
    pub fn new(cx: &mut App) -> Self {
        Self {
            available_update: cx.new(|_cx| None),
        }
    }

    pub fn check_for_updates(
        http_client: Arc<dyn HttpClient>,
        available_update: Entity<Option<ReleaseInfo>>,
        cx: &mut App,
    ) {
        Self::poll_for_updates(http_client, available_update, false, cx);
    }

    pub fn apply_update(
        http_client: Arc<dyn HttpClient>,
        available_update: Entity<Option<ReleaseInfo>>,
        cx: &mut App,
    ) {
        let is_stale = available_update
            .read(cx)
            .as_ref()
            .map(|info| info.found_at.elapsed() >= CHECK_INTERVAL)
            .unwrap_or(true);

        cx.spawn(async move |mut cx| {
            if is_stale {
                match Self::fetch_latest_release(&http_client).await {
                    Ok(Some(info)) => set_available_update(&available_update, Some(info), &mut cx),
                    _ => {
                        set_available_update(&available_update, None, &mut cx);
                        return;
                    }
                }
            }

            let _ = smol::unblock(|| {
                let mut builder = self_update::backends::github::Update::configure();
                builder
                    .repo_owner("astrum-chat")
                    .repo_name("astrum")
                    .bin_name("astrum")
                    .current_version(env!("CARGO_PKG_VERSION"))
                    .no_confirm(true)
                    .show_output(false)
                    .show_download_progress(false);

                configure_platform_target(&mut builder);

                builder.build().and_then(|updater| updater.update())
            })
            .await;
        })
        .detach();
    }

    fn poll_for_updates(
        http_client: Arc<dyn HttpClient>,
        available_update: Entity<Option<ReleaseInfo>>,
        initial_delay: bool,
        cx: &mut App,
    ) {
        cx.spawn(async move |mut cx| {
            if initial_delay {
                smol::Timer::after(CHECK_INTERVAL).await;
            }

            loop {
                if let Ok(Some(info)) = Self::fetch_latest_release(&http_client).await {
                    set_available_update(&available_update, Some(info), &mut cx);
                    return;
                }

                smol::Timer::after(CHECK_INTERVAL).await;
            }
        })
        .detach();
    }

    async fn fetch_latest_release(
        http_client: &Arc<dyn HttpClient>,
    ) -> anyhow::Result<Option<ReleaseInfo>> {
        let request = http::Request::builder()
            .method("GET")
            .uri("https://api.github.com/repos/astrum-chat/astrum/releases/latest")
            .header("User-Agent", "astrum")
            .header("Accept", "application/vnd.github+json")
            .body(AsyncBody::empty())?;

        let response = http_client.send(request).await?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let mut body = Vec::new();
        response.into_body().read_to_end(&mut body).await?;

        let json: serde_json::Value = serde_json::from_slice(&body)?;
        let tag_name = json["tag_name"].as_str().unwrap_or_default();
        let html_url = json["html_url"].as_str().unwrap_or_default();

        let remote_version_str = tag_name.strip_prefix('v').unwrap_or(tag_name);
        let remote_version = Version::parse(remote_version_str)?;
        let current_version = Version::parse(env!("CARGO_PKG_VERSION"))?;

        if remote_version > current_version {
            Ok(Some(ReleaseInfo {
                version: tag_name.to_string(),
                html_url: html_url.to_string(),
                found_at: Instant::now(),
            }))
        } else {
            Ok(None)
        }
    }
}

fn set_available_update(
    entity: &Entity<Option<ReleaseInfo>>,
    value: Option<ReleaseInfo>,
    cx: &mut gpui::AsyncApp,
) {
    let _ = entity.update(cx, |state, cx| {
        *state = value;
        cx.notify();
    });
}

fn configure_platform_target(builder: &mut self_update::backends::github::UpdateBuilder) {
    if cfg!(target_os = "macos") {
        let macos_identifier = if cfg!(HAS_LIQUID_GLASS_WINDOW) {
            "tahoe"
        } else {
            "sequoia"
        };
        builder
            .target("macos")
            .identifier(macos_identifier)
            .bin_path_in_archive("astrum");
    } else if cfg!(target_os = "linux") {
        builder.target("linux");
    } else if cfg!(target_os = "windows") {
        builder.target("windows");
    }
}
