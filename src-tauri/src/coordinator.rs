//! Single-threaded owner of the session state machine.
//!
//! Hotkey edges and recognizer events both arrive on one channel, so the
//! machine never sees concurrent transitions.

use std::collections::HashMap;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, watch};
use type4we_core::asr::{
    create_engine, drive_session, AsrCredentials, AsrSessionOptions, DriveSession, EngineEvent,
};
use type4we_core::hotkey::{activation_command, Binding};
use type4we_core::secrets::{
    ACCESS_KEY_FIELD, API_KEY_FIELD, APP_KEY_FIELD, SONIOX_ACCOUNT, VOLCANO_ACCOUNT,
};
use type4we_core::session::{Effect, PhaseKind, Session, SessionEvent};

use crate::commands::{store_status, SessionView, StatusCell};
use crate::messages::CoordCommand;
use crate::platform::{self, SharedState};

#[derive(Clone, serde::Serialize)]
struct SessionPayload {
    phase: String,
    preview: String,
    error: Option<String>,
}

pub async fn run(
    app: AppHandle,
    shared: SharedState,
    mut commands: mpsc::UnboundedReceiver<CoordCommand>,
) {
    let mut session = Session::new();
    let mut preview = String::new();
    let mut stops: HashMap<u64, watch::Sender<bool>> = HashMap::new();

    publish(&app, &shared, &session, &preview);

    while let Some(command) = commands.recv().await {
        match command {
            CoordCommand::Hotkey(edge) => {
                let (activation, open) = {
                    let config = shared.config.read().unwrap_or_else(|err| err.into_inner());
                    (
                        config.hotkey.activation,
                        !matches!(session.phase.kind(), PhaseKind::Idle),
                    )
                };
                let Some(start) = activation_command(activation, edge, open) else {
                    continue;
                };
                let event = if start {
                    SessionEvent::Start
                } else {
                    SessionEvent::Stop
                };
                apply(&app, &shared, &mut session, &mut preview, &mut stops, event);
            }
            CoordCommand::Engine { id, event } => {
                if let EngineEvent::Update { preview: next, .. } = &event {
                    preview = next.clone();
                }
                let session_event = match event {
                    EngineEvent::Ready => SessionEvent::EngineReady { id },
                    EngineEvent::Update { newly_final, .. } => {
                        SessionEvent::Transcript { id, newly_final }
                    }
                    EngineEvent::Finished { remainder } => {
                        stops.remove(&id);
                        SessionEvent::Finished { id, remainder }
                    }
                    EngineEvent::Failed(message) => {
                        stops.remove(&id);
                        SessionEvent::EngineFailed { id, message }
                    }
                };
                apply(
                    &app,
                    &shared,
                    &mut session,
                    &mut preview,
                    &mut stops,
                    session_event,
                );
                if matches!(session.phase.kind(), PhaseKind::Idle) {
                    preview.clear();
                }
                publish(&app, &shared, &session, &preview);
            }
        }
    }
}

fn apply(
    app: &AppHandle,
    shared: &SharedState,
    session: &mut Session,
    preview: &mut String,
    stops: &mut HashMap<u64, watch::Sender<bool>>,
    event: SessionEvent,
) {
    let effects = session.handle(event);
    for effect in effects {
        match effect {
            Effect::BeginSession { id } => {
                let (stop_tx, stop_rx) = watch::channel(false);
                stops.insert(id, stop_tx);
                spawn_engine(app.clone(), shared.clone(), id, stop_rx);
            }
            Effect::EndSession { id } => {
                if let Some(stop) = stops.get(&id) {
                    let _ = stop.send(true);
                }
            }
            Effect::Inject(text) => {
                let (method, binding) = {
                    let config = shared.config.read().unwrap_or_else(|err| err.into_inner());
                    let binding = config.binding().unwrap_or_else(|_| Binding::right_alt());
                    (config.injection.method, binding)
                };
                if let Err(err) = platform::inject_text(&text, method, &binding) {
                    tracing::warn!(error = %err, "text injection failed");
                    let follow = session.handle(SessionEvent::InjectFailed { message: err });
                    for effect in follow {
                        if let Effect::EndSession { id } = effect {
                            if let Some(stop) = stops.get(&id) {
                                let _ = stop.send(true);
                            }
                        }
                    }
                }
            }
        }
    }
    if matches!(session.phase.kind(), PhaseKind::Idle) {
        preview.clear();
    }
    publish(app, shared, session, preview);
}

fn spawn_engine(app: AppHandle, shared: SharedState, id: u64, stop: watch::Receiver<bool>) {
    let commands = shared.commands.clone();
    tauri::async_runtime::spawn(async move {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let forward = commands.clone();
        let bridge = tauri::async_runtime::spawn(async move {
            while let Some(event) = rx.recv().await {
                if forward.send(CoordCommand::Engine { id, event }).is_err() {
                    break;
                }
            }
        });
        if let Err(err) = run_engine(shared, stop, tx.clone()).await {
            let _ = tx.send(EngineEvent::Failed(err));
        }
        drop(tx);
        let _ = bridge.await;
        let _ = app;
    });
}

async fn run_engine(
    shared: SharedState,
    stop: watch::Receiver<bool>,
    events: mpsc::UnboundedSender<EngineEvent>,
) -> Result<(), String> {
    let (provider, model, hints, sensitivity, resource_id, auth_mode, device) = {
        let config = shared.config.read().map_err(|err| err.to_string())?;
        (
            config.selected_asr_provider.clone(),
            config.soniox.model.clone(),
            config.soniox.language_hints.clone(),
            config.endpoint_sensitivity(),
            config.volcano.resource_id.clone(),
            config.volcano.auth_mode.clone(),
            config.audio.input_device.clone(),
        )
    };
    let mut credentials = AsrCredentials::default();
    match provider.as_str() {
        "volcano" => {
            for field in [APP_KEY_FIELD, ACCESS_KEY_FIELD, API_KEY_FIELD] {
                if let Some(value) = shared
                    .secrets
                    .get(VOLCANO_ACCOUNT, field)
                    .map_err(|err| err.to_string())?
                    .filter(|value| !value.trim().is_empty())
                {
                    credentials.values.insert(field.to_string(), value);
                }
            }
            let ready = if auth_mode == "apiKey" {
                credentials.get(API_KEY_FIELD).is_some()
            } else {
                credentials.get(APP_KEY_FIELD).is_some()
                    && credentials.get(ACCESS_KEY_FIELD).is_some()
            };
            if !ready {
                return Err(
                    "Volcengine credentials are not set. Add the App ID and Access Token in Settings."
                        .into(),
                );
            }
        }
        "soniox" => {
            let api_key = shared
                .secrets
                .get(SONIOX_ACCOUNT, API_KEY_FIELD)
                .map_err(|err| err.to_string())?
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Soniox API key is not set. Add it in Settings.".to_string())?;
            credentials
                .values
                .insert(API_KEY_FIELD.to_string(), api_key);
        }
        other => {
            return Err(format!(
                "ASR provider `{other}` is not available. Choose Volcengine or Soniox in Settings."
            ));
        }
    }

    if *stop.borrow() {
        let _ = events.send(EngineEvent::Finished {
            remainder: String::new(),
        });
        return Ok(());
    }

    let engine = create_engine(&provider).map_err(|err| err.to_string())?;
    let requirements = engine.audio_requirements();
    let capture = platform::start_capture(device.as_deref()).map_err(|err| err.to_string())?;
    let (writer, reader) = engine
        .open(
            &credentials,
            &AsrSessionOptions {
                model,
                language_hints: hints,
                endpoint_sensitivity: sensitivity,
                resource_id,
                auth_mode,
            },
        )
        .await
        .map_err(|err| err.to_string())?;

    if *stop.borrow() {
        let _ = events.send(EngineEvent::Finished {
            remainder: String::new(),
        });
        return Ok(());
    }

    let platform::Capture {
        sample_rate,
        audio,
        release: _release,
    } = capture;
    drive_session(DriveSession {
        writer,
        reader,
        audio,
        input_sample_rate: sample_rate,
        target: requirements,
        stop,
        events,
    })
    .await;
    drop(_release);
    Ok(())
}

fn publish(app: &AppHandle, shared: &SharedState, session: &Session, preview: &str) {
    let error = session.last_error.clone();
    let phase = session.phase.kind();
    let payload = SessionPayload {
        phase: phase_label(phase),
        preview: preview.to_string(),
        error: error.clone(),
    };
    let _ = app.emit("session-update", &payload);
    if let Some(cell) = app.try_state::<StatusCell>() {
        store_status(
            &cell,
            SessionView {
                phase: payload.phase.clone(),
                preview: payload.preview.clone(),
                error: payload.error.clone(),
            },
        );
    }
    update_tray(app, shared, phase, error.as_deref());
    update_overlay(app, shared, phase, preview);
}

fn phase_label(phase: PhaseKind) -> String {
    match phase {
        PhaseKind::Idle => "idle",
        PhaseKind::Starting => "starting",
        PhaseKind::Listening => "listening",
        PhaseKind::Finalizing => "finalizing",
    }
    .into()
}

fn update_tray(app: &AppHandle, shared: &SharedState, phase: PhaseKind, error: Option<&str>) {
    let lang = {
        let config = shared.config.read().unwrap_or_else(|err| err.into_inner());
        config.language.clone()
    };
    let status = match (phase, error) {
        (_, Some(_)) if matches!(phase, PhaseKind::Idle) => {
            tr(&lang, "Status: Error", "状态：出错")
        }
        (PhaseKind::Idle, _) => tr(&lang, "Status: Ready", "状态：就绪"),
        (PhaseKind::Starting, _) => tr(&lang, "Status: Starting", "状态：启动中"),
        (PhaseKind::Listening, _) => tr(&lang, "Status: Listening", "状态：聆听中"),
        (PhaseKind::Finalizing, _) => tr(&lang, "Status: Finishing", "状态：结束中"),
    };
    let mode = tr(&lang, "Mode: Quick", "模式：快速");
    let app = app.clone();
    let tray = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(state) = tray.try_state::<TrayHandles>() {
            let _ = state.status.set_text(status);
            let _ = state.mode.set_text(mode);
            let _ = state.settings.set_text(tr(&lang, "Settings", "设置"));
            let _ = state.quit.set_text(tr(&lang, "Quit", "退出"));
        }
    });
}

fn update_overlay(app: &AppHandle, shared: &SharedState, phase: PhaseKind, preview: &str) {
    let show = {
        let config = shared.config.read().unwrap_or_else(|err| err.into_inner());
        config.overlay.show_partials
    };
    let active = matches!(
        phase,
        PhaseKind::Listening | PhaseKind::Finalizing | PhaseKind::Starting
    );
    let Some(window) = app.get_webview_window("overlay") else {
        return;
    };
    if !show || !active || preview.is_empty() {
        let _ = window.hide();
        return;
    }
    if let Some((x, y)) = platform::cursor_pos() {
        let _ = window.set_position(tauri::PhysicalPosition::new(x + 16, y + 24));
    }
    let _ = window.emit("overlay-text", preview.to_string());
    let _ = window.show();
}

pub fn tr(lang: &str, en: &str, zh: &str) -> String {
    if lang.to_ascii_lowercase().starts_with("zh") {
        zh.to_string()
    } else {
        en.to_string()
    }
}

pub struct TrayHandles {
    pub status: tauri::menu::MenuItem<tauri::Wry>,
    pub mode: tauri::menu::MenuItem<tauri::Wry>,
    pub settings: tauri::menu::MenuItem<tauri::Wry>,
    pub quit: tauri::menu::MenuItem<tauri::Wry>,
}
