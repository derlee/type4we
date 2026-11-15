mod commands;
mod coordinator;
mod messages;
mod platform;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;
use type4we_core::config;
use type4we_core::hotkey::ChordTracker;

use commands::{SessionView, StatusCell};
use coordinator::TrayHandles;
use messages::CoordCommand;
use platform::SharedState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let dir = app_dir();
    let _ = fs::create_dir_all(dir.join("logs"));
    init_logging(&dir);

    tauri::Builder::default()
        .setup(move |app| {
            let (shared, rx) = load_shared(&dir)?;
            if let Err(err) = platform::install_hotkey(shared.clone()) {
                tracing::error!(error = %err, "global hotkey was not installed");
            }
            let menu = build_menu(app.handle(), &shared)?;
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?;
            let _tray = TrayIconBuilder::with_id("main")
                .icon(icon)
                .tooltip("Type4We")
                .menu(&menu.menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "settings" => show_settings(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            app.manage(menu.handles);
            app.manage(StatusCell(Mutex::new(SessionView {
                phase: "idle".into(),
                preview: String::new(),
                error: None,
            })));
            app.manage(shared.clone());
            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay.set_focusable(false);
            }
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                coordinator::run(handle, shared, rx).await;
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::set_api_key,
            commands::clear_api_key,
            commands::set_volc_credentials,
            commands::secret_status,
            commands::list_microphones,
            commands::session_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("Type4We failed to start");
}

struct MenuBundle {
    menu: Menu<tauri::Wry>,
    handles: TrayHandles,
}

fn build_menu(app: &tauri::AppHandle, shared: &SharedState) -> tauri::Result<MenuBundle> {
    let lang = shared
        .config
        .read()
        .map(|config| config.language.clone())
        .unwrap_or_else(|_| "en".into());
    let status = MenuItem::with_id(
        app,
        "status",
        coordinator::tr(&lang, "Status: Ready", "状态：就绪"),
        false,
        None::<&str>,
    )?;
    let mode = MenuItem::with_id(
        app,
        "mode",
        coordinator::tr(&lang, "Mode: Quick", "模式：快速"),
        false,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(
        app,
        "settings",
        coordinator::tr(&lang, "Settings", "设置"),
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        coordinator::tr(&lang, "Quit", "退出"),
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &mode,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &quit,
        ],
    )?;
    Ok(MenuBundle {
        menu,
        handles: TrayHandles {
            status,
            mode,
            settings,
            quit,
        },
    })
}

fn show_settings(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn load_shared(dir: &Path) -> Result<(SharedState, mpsc::UnboundedReceiver<CoordCommand>), String> {
    let config_path = dir.join("config.json");
    let outcome = config::load_from_path(&config_path).map_err(|err| err.to_string())?;
    for warning in &outcome.warnings {
        tracing::warn!(warning = %warning, "config");
    }
    let secrets = platform::windows_secret_store();
    if outcome.stripped_secrets {
        for secret in &outcome.extracted_secrets {
            if let Err(err) = secrets.set(&secret.account, &secret.field, &secret.value) {
                tracing::error!(error = %err, account = %secret.account, "could not move a secret out of config.json");
            }
        }
        config::save_to_path(&config_path, &outcome.config).map_err(|err| err.to_string())?;
        tracing::info!("removed API keys from config.json and stored them in Credential Manager");
    }
    let binding = outcome.config.binding().unwrap_or_else(|_| {
        tracing::warn!("falling back to Right Alt");
        type4we_core::hotkey::Binding::right_alt()
    });
    let (tx, rx) = mpsc::unbounded_channel();
    let shared = SharedState {
        config: Arc::new(RwLock::new(outcome.config)),
        config_path,
        tracker: Arc::new(Mutex::new(ChordTracker::new(binding))),
        secrets,
        commands: tx,
    };
    Ok((shared, rx))
}

fn app_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("Type4We")
    } else {
        PathBuf::from(".").join("Type4We")
    }
}

fn init_logging(dir: &Path) {
    let log_path = dir.join("logs").join("type4we.log");
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if let Ok(file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(file)
            .with_ansi(false)
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
    }
}
