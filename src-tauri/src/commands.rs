use std::sync::Mutex;

use tauri::State;
use type4we_core::config::{self, Config};
use type4we_core::hotkey::{Activation, Binding};
use type4we_core::inject::InjectionMethod;
use type4we_core::secrets::{
    ACCESS_KEY_FIELD, API_KEY_FIELD, APP_KEY_FIELD, SONIOX_ACCOUNT, VOLCANO_ACCOUNT,
};

use crate::platform::{self, SharedState};

#[derive(serde::Serialize)]
pub struct SettingsView {
    pub binding: String,
    pub activation: String,
    pub injection_method: String,
    pub show_partials: bool,
    pub input_device: Option<String>,
    pub model: String,
    pub endpoint_sensitivity: f64,
    pub language_hints: String,
    pub ui_language: String,
    pub provider: String,
    pub api_key_set: bool,
    pub volc_auth_mode: String,
    pub volc_resource_id: String,
    pub volc_app_id_set: bool,
    pub volc_access_token_set: bool,
    pub volc_api_key_set: bool,
    pub config_path: String,
}

#[derive(serde::Deserialize)]
pub struct SettingsUpdate {
    pub binding: String,
    pub activation: String,
    pub injection_method: String,
    pub show_partials: bool,
    pub input_device: Option<String>,
    pub model: String,
    pub endpoint_sensitivity: f64,
    pub language_hints: String,
    pub ui_language: String,
    pub provider: String,
    pub volc_auth_mode: String,
    pub volc_resource_id: String,
}

#[derive(serde::Serialize)]
pub struct SecretStatus {
    pub set: bool,
}

#[derive(serde::Serialize, Clone)]
pub struct SessionView {
    pub phase: String,
    pub preview: String,
    pub error: Option<String>,
}

pub struct StatusCell(pub Mutex<SessionView>);

#[tauri::command]
pub fn get_settings(shared: State<SharedState>) -> Result<SettingsView, String> {
    let config = shared.config.read().map_err(|err| err.to_string())?;
    let api_key_set = secret_set(&shared, SONIOX_ACCOUNT, API_KEY_FIELD)?;
    let volc_app_id_set = secret_set(&shared, VOLCANO_ACCOUNT, APP_KEY_FIELD)?;
    let volc_access_token_set = secret_set(&shared, VOLCANO_ACCOUNT, ACCESS_KEY_FIELD)?;
    let volc_api_key_set = secret_set(&shared, VOLCANO_ACCOUNT, API_KEY_FIELD)?;
    Ok(view(
        &config,
        api_key_set,
        volc_app_id_set,
        volc_access_token_set,
        volc_api_key_set,
        &shared.config_path.display().to_string(),
    ))
}

#[tauri::command]
pub fn save_settings(
    shared: State<SharedState>,
    update: SettingsUpdate,
) -> Result<SettingsView, String> {
    let _binding = Binding::parse(&update.binding).map_err(|err| err.to_string())?;
    let activation = Activation::parse(&update.activation)
        .ok_or_else(|| "activation must be push_to_talk or toggle".to_string())?;
    let method = InjectionMethod::parse(&update.injection_method)
        .ok_or_else(|| "injection method must be auto, unicode, or clipboard".to_string())?;
    if !(-1.0..=1.0).contains(&update.endpoint_sensitivity) {
        return Err("endpoint sensitivity must be between -1 and 1".into());
    }
    {
        let mut config = shared.config.write().map_err(|err| err.to_string())?;
        config.hotkey.binding = update.binding.trim().to_string();
        config.hotkey.activation = activation;
        config.injection.method = method;
        config.overlay.show_partials = update.show_partials;
        config.audio.input_device = update.input_device.filter(|value| !value.trim().is_empty());
        config.soniox.model = update.model;
        config.soniox.endpoint_sensitivity = update.endpoint_sensitivity.to_string();
        config.soniox.language_hints = update
            .language_hints
            .split([',', ' '])
            .map(|hint| hint.trim().to_string())
            .filter(|hint| !hint.is_empty())
            .collect();
        config.language = update.ui_language;
        config.selected_asr_provider = match update.provider.trim() {
            "soniox" => "soniox".into(),
            _ => "volcano".into(),
        };
        if !update.volc_resource_id.trim().is_empty() {
            config.volcano.resource_id = update.volc_resource_id.trim().to_string();
        }
        if matches!(update.volc_auth_mode.trim(), "legacy" | "apiKey") {
            config.volcano.auth_mode = update.volc_auth_mode.trim().to_string();
        }
        config.mode = "quick".into();
        config.normalize();
        config::save_to_path(&shared.config_path, &config).map_err(|err| err.to_string())?;
        let parsed = config.binding().map_err(|err| err.to_string())?;
        drop(config);
        shared
            .tracker
            .lock()
            .map_err(|err| err.to_string())?
            .set_binding(parsed);
    }
    get_settings(shared)
}

#[tauri::command]
pub fn set_api_key(shared: State<SharedState>, api_key: String) -> Result<SecretStatus, String> {
    let api_key = api_key.trim().to_string();
    if api_key.is_empty() {
        return Err("API key is empty".into());
    }
    shared
        .secrets
        .set(SONIOX_ACCOUNT, API_KEY_FIELD, &api_key)
        .map_err(|err| err.to_string())?;
    Ok(SecretStatus { set: true })
}

#[tauri::command]
pub fn clear_api_key(shared: State<SharedState>) -> Result<SecretStatus, String> {
    shared
        .secrets
        .delete(SONIOX_ACCOUNT, API_KEY_FIELD)
        .map_err(|err| err.to_string())?;
    Ok(SecretStatus { set: false })
}

#[tauri::command]
pub fn secret_status(shared: State<SharedState>) -> Result<SecretStatus, String> {
    let set = shared
        .secrets
        .get(SONIOX_ACCOUNT, API_KEY_FIELD)
        .map_err(|err| err.to_string())?
        .is_some();
    Ok(SecretStatus { set })
}

#[tauri::command]
pub fn list_microphones() -> Vec<String> {
    platform::list_input_devices()
}

#[tauri::command]
pub fn session_snapshot(status: State<StatusCell>) -> SessionView {
    status
        .0
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or(SessionView {
            phase: "idle".into(),
            preview: String::new(),
            error: None,
        })
}

pub fn store_status(cell: &StatusCell, view: SessionView) {
    if let Ok(mut guard) = cell.0.lock() {
        *guard = view;
    }
}

fn secret_set(shared: &SharedState, account: &str, field: &str) -> Result<bool, String> {
    Ok(shared
        .secrets
        .get(account, field)
        .map_err(|err| err.to_string())?
        .is_some())
}

#[tauri::command]
pub fn set_volc_credentials(
    shared: State<SharedState>,
    app_id: String,
    access_token: String,
    api_key: String,
) -> Result<(), String> {
    store_if_present(&shared, APP_KEY_FIELD, &app_id)?;
    store_if_present(&shared, ACCESS_KEY_FIELD, &access_token)?;
    store_if_present(&shared, API_KEY_FIELD, &api_key)?;
    Ok(())
}

fn store_if_present(shared: &SharedState, field: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(());
    }
    shared
        .secrets
        .set(VOLCANO_ACCOUNT, field, value)
        .map_err(|err| err.to_string())
}

fn view(
    config: &Config,
    api_key_set: bool,
    volc_app_id_set: bool,
    volc_access_token_set: bool,
    volc_api_key_set: bool,
    path: &str,
) -> SettingsView {
    SettingsView {
        binding: config.hotkey.binding.clone(),
        activation: config.hotkey.activation.as_str().into(),
        injection_method: config.injection.method.as_str().into(),
        show_partials: config.overlay.show_partials,
        input_device: config.audio.input_device.clone(),
        model: config.soniox.model.clone(),
        endpoint_sensitivity: config.endpoint_sensitivity(),
        language_hints: config.soniox.language_hints.join(", "),
        ui_language: config.language.clone(),
        provider: config.selected_asr_provider.clone(),
        api_key_set,
        volc_auth_mode: config.volcano.auth_mode.clone(),
        volc_resource_id: config.volcano.resource_id.clone(),
        volc_app_id_set,
        volc_access_token_set,
        volc_api_key_set,
        config_path: path.into(),
    }
}
