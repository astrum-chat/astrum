use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::AsyncReadExt;
use gpui::{
    App, AppContext, Entity,
    http_client::{AsyncBody, HttpClient},
};
use semver::Version;

const CHECK_INTERVAL: Duration = Duration::from_secs(30 * 60);
const CHECK_INTERVAL_STAGED: Duration = Duration::from_secs(60 * 60);

#[derive(Clone)]
pub struct ReleaseInfo {
    pub version: String,
    pub html_url: String,
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

    pub fn apply_pending_update() {
        let staged = staged_binary_path();
        if !staged.exists() {
            tracing::warn!("no staged update found at {}", staged.display());
            return;
        }

        if let Err(e) = self_update::self_replace::self_replace(&staged) {
            tracing::error!("failed to apply update: {e}");
            return;
        }

        let _ = fs::remove_file(&staged);
        tracing::info!("update applied, restarting...");
        restart_app();
    }

    fn poll_for_updates(
        http_client: Arc<dyn HttpClient>,
        available_update: Entity<Option<ReleaseInfo>>,
        initial_delay: bool,
        cx: &mut App,
    ) {
        cx.spawn(async move |mut cx| {
            let already_staged = staged_binary_path().exists();

            if already_staged {
                tracing::info!("found previously staged update");
                if let Ok(Some(info)) = Self::fetch_latest_release(&http_client).await {
                    set_available_update(&available_update, Some(info), &mut cx);
                } else {
                    set_available_update(
                        &available_update,
                        Some(ReleaseInfo {
                            version: "update".to_string(),
                            html_url: String::new(),
                        }),
                        &mut cx,
                    );
                }

                loop {
                    smol::Timer::after(CHECK_INTERVAL_STAGED).await;

                    if let Ok(Some(info)) = Self::fetch_latest_release(&http_client).await {
                        let staged_version = get_staged_version(&available_update, &mut cx);
                        let is_newer = is_version_newer(staged_version.as_deref(), &info.version);

                        if is_newer {
                            tracing::info!(
                                "newer update found: {}, re-downloading...",
                                info.version
                            );
                            let result = smol::unblock(|| download_update_to_staging()).await;
                            match result {
                                Ok(()) => {
                                    tracing::info!("newer update staged successfully");
                                    set_available_update(&available_update, Some(info), &mut cx);
                                }
                                Err(e) => tracing::error!("failed to stage newer update: {e}"),
                            }
                        }
                    }
                }
            }

            if initial_delay {
                smol::Timer::after(CHECK_INTERVAL).await;
            }

            loop {
                if let Ok(Some(info)) = Self::fetch_latest_release(&http_client).await {
                    tracing::info!("update found: {}, downloading...", info.version);

                    let result = smol::unblock(|| download_update_to_staging()).await;

                    match result {
                        Ok(()) => {
                            tracing::info!("update staged successfully");
                            set_available_update(&available_update, Some(info), &mut cx);
                            return;
                        }
                        Err(e) => tracing::error!("failed to stage update: {e}"),
                    }
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
            }))
        } else {
            Ok(None)
        }
    }
}

fn updates_dir() -> PathBuf {
    dirs::data_local_dir()
        .expect("failed to get local data dir")
        .join("chat.astrum.astrum")
        .join("updates")
}

fn staged_binary_path() -> PathBuf {
    let name = if cfg!(target_os = "windows") {
        "astrum.exe"
    } else {
        "astrum"
    };
    updates_dir().join(name)
}

fn download_update_to_staging() -> anyhow::Result<()> {
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

    let updater = builder.build()?;
    let release = updater.get_latest_release()?;
    let target = updater.target();
    let identifier = updater.identifier();
    let update_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else if cfg!(target_os = "macos") {
        ".tar.gz"
    } else {
        ".AppImage"
    };
    let asset = release
        .assets
        .iter()
        .find(|a| {
            a.name.contains(&target)
                && identifier.as_deref().map_or(true, |id| a.name.contains(id))
                && a.name.ends_with(update_ext)
        })
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no asset found for target: {target}"))?;

    let updates = updates_dir();
    fs::create_dir_all(&updates)?;

    let tmp_archive_path = updates.join(&asset.name);
    let mut tmp_archive = fs::File::create(&tmp_archive_path)?;

    let mut download = self_update::Download::from_url(&asset.download_url);
    download.set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?);
    download.show_progress(false);
    download.download_to(&mut tmp_archive)?;

    let bin_name = if cfg!(target_os = "windows") {
        "astrum.exe"
    } else {
        "astrum"
    };

    self_update::Extract::from_source(&tmp_archive_path).extract_file(&updates, bin_name)?;

    let _ = fs::remove_file(&tmp_archive_path);

    if !staged_binary_path().exists() {
        anyhow::bail!("staged binary not found after extraction");
    }

    Ok(())
}

fn get_staged_version(
    entity: &Entity<Option<ReleaseInfo>>,
    cx: &mut gpui::AsyncApp,
) -> Option<String> {
    entity.update(cx, |state, _cx| state.as_ref().map(|s| s.version.clone()))
}

fn is_version_newer(current: Option<&str>, candidate: &str) -> bool {
    let Some(current) = current else {
        return true;
    };
    let current_v = current.strip_prefix('v').unwrap_or(current);
    let candidate_v = candidate.strip_prefix('v').unwrap_or(candidate);
    match (Version::parse(candidate_v), Version::parse(current_v)) {
        (Ok(new), Ok(old)) => new > old,
        _ => false,
    }
}

fn set_available_update(
    entity: &Entity<Option<ReleaseInfo>>,
    value: Option<ReleaseInfo>,
    cx: &mut gpui::AsyncApp,
) {
    entity.update(cx, |state, cx| {
        *state = value;
        cx.notify();
    });
}

fn restart_app() {
    let exe = std::env::current_exe().expect("failed to get current exe path");
    std::process::Command::new(exe)
        .spawn()
        .expect("failed to restart app");
    std::process::exit(0);
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
