use gpui::{
    App, Application, Bounds, KeyBinding, Menu, MenuItem, SharedString, TitlebarOptions,
    WindowBounds, WindowOptions, actions, point, prelude::*, px, size,
};
use gpui_tesserae::{
    TesseraeAssets, assets,
    theme::{Theme, ThemeExt},
    views::Root,
};
use smol::lock::RwLock;
use std::sync::Arc;

use crate::{
    blocks::models_menu::prefetch_all_models,
    managers::{Managers, UpdateManager},
    views::SettingsView,
};

mod assets;
use assets::AstrumAssets;

mod views;
use views::ChatView;

pub mod blocks;

mod anyhttp_gpui;

mod http_client;
pub use http_client::ReqwestHttpClient;

mod utils;
pub use utils::*;

mod managers;

actions!(window, [TabNext, TabPrev, OpenSettings]);

fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    Application::new()
        .with_quit_mode(gpui::QuitMode::LastWindowClosed)
        .with_assets(assets![AstrumAssets, TesseraeAssets])
        .run(|cx: &mut App| {
            gpui_tesserae::init(cx);
            cx.set_theme(Theme::DEFAULT);

            cx.set_http_client(Arc::new(ReqwestHttpClient::new().unwrap()));

            let managers = Arc::new(RwLock::new(Managers::new(cx)));

            {
                let managers = managers.clone();

                let min_size = size(px(650.), px(450.));
                let initial_size = size(px(950.), px(750.));
                let bounds = Bounds::centered(None, initial_size, cx);

                cx.spawn(async move |cx| {
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(bounds)),
                            window_min_size: Some(min_size),
                            titlebar: Some(TitlebarOptions {
                                title: Some(SharedString::new("Astrum")),
                                appears_transparent: true,
                                traffic_light_position: Some(point(px(10.), px(10.))),
                            }),
                            ..Default::default()
                        },
                        |window, cx| {
                            let chat_view = cx.new(move |cx| {
                                let chat_view = ChatView::new("chat_view", managers);

                                cx.spawn(async |chat_view, cx| {
                                    let _ = chat_view.update(cx, |chat_view: &mut ChatView, cx| {
                                        let _ = chat_view.managers.write_arc_blocking().init(cx);

                                        prefetch_all_models(chat_view.managers.clone(), cx);

                                        let http_client = cx.http_client();
                                        let available_update = chat_view
                                            .managers
                                            .read_blocking()
                                            .update
                                            .available_update
                                            .clone();
                                        UpdateManager::check_for_updates(
                                            http_client,
                                            available_update,
                                            cx,
                                        );
                                    });
                                })
                                .detach();

                                cx.activate(true);

                                chat_view
                            });

                            cx.new(|cx| Root::new(chat_view, window, cx))
                        },
                    )?;

                    Ok::<_, anyhow::Error>(())
                })
                .detach();
            }

            cx.set_menus(vec![
                Menu {
                    name: "Astrum".into(),
                    items: vec![MenuItem::action("Settings", OpenSettings)],
                },
                Menu {
                    name: "Window".into(),
                    items: vec![],
                },
            ]);

            init_tab_indexing_actions(cx);
            init_open_settings_action(cx, managers);
        });
}

fn init_tab_indexing_actions(cx: &mut App) {
    cx.on_action(move |_: &TabNext, cx| {
        cx.defer(move |cx| {
            let Some(window) = cx.active_window() else {
                return;
            };

            let _ = window.update(cx, move |_, window, cx| {
                window.focus_next(cx);
            });
        })
    });

    cx.on_action(move |_: &TabPrev, cx| {
        cx.defer(move |cx| {
            let Some(window) = cx.active_window() else {
                return;
            };

            let _ = window.update(cx, move |_, window, cx| {
                window.focus_prev(cx);
            });
        })
    });

    cx.bind_keys([KeyBinding::new("tab", TabNext, None)]);
    cx.bind_keys([KeyBinding::new("shift-tab", TabPrev, None)]);
}

fn init_open_settings_action(cx: &mut App, managers: Arc<RwLock<Managers>>) {
    cx.on_action(move |_: &OpenSettings, cx| {
        if !open_existing_window(cx) {
            open_new_settings_window(cx, managers.clone())
        }
    });
}

fn open_existing_window(cx: &mut App) -> bool {
    let Some(settings_window) = Root::find_window::<SettingsView>(cx) else {
        return false;
    };

    let _ = settings_window.update(cx, |_view, window, _cx| {
        window.activate_window();
    });

    return true;
}

fn open_new_settings_window(cx: &mut App, managers: Arc<RwLock<Managers>>) {
    let min_size = size(px(650.), px(450.));
    let initial_size = size(px(750.), px(550.));
    let bounds = Bounds::centered(None, initial_size, cx);

    cx.spawn(async move |cx| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(min_size),
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from("Astrum - Settings")),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(10.), px(10.))),
                }),
                ..Default::default()
            },
            |window, cx| {
                let settings_view = cx.new(move |_cx| SettingsView::new("settings_view", managers));

                cx.new(|cx| Root::new(settings_view, window, cx))
            },
        )?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}
