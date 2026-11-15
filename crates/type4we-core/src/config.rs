//! JSON config in `%APPDATA%\Type4We\config.json`.
//!
//! The `tf_*` keys follow Type4Me so a later import of the Mac
//! `credentials.json` can land in the same shape. Secure fields (`apiKey`,
//! `accessKey`, and the other names in [`SECRET_FIELDS`]) are removed on load
//! and returned to the caller, which stores them in Credential Manager.

use crate::hotkey::{Activation, Binding};
use crate::inject::InjectionMethod;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const CONFIG_VERSION: u32 = 1;
pub const DEFAULT_MODEL: &str = "stt-rt-v6";
pub const DEFAULT_BINDING: &str = "RAlt";
pub const DEFAULT_VOLC_RESOURCE: &str = "volc.seedasr.sauc.duration";

const SECRET_FIELDS: &[&str] = &[
    "apiKey",
    "api_key",
    "accessKey",
    "access_key",
    "appKey",
    "token",
    "secret",
    "secretKey",
    "stepPlanApiKey",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyConfig {
    pub binding: String,
    pub activation: Activation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectionConfig {
    pub method: InjectionMethod,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioConfig {
    #[serde(default)]
    pub input_device: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayConfig {
    pub show_partials: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SonioxSettings {
    #[serde(default = "default_model")]
    pub model: String,
    /// Type4Me stores this credential as a string. Numbers are accepted on load.
    #[serde(
        default = "default_sensitivity",
        rename = "endpointSensitivity",
        deserialize_with = "de_flex_string"
    )]
    pub endpoint_sensitivity: String,
    #[serde(default = "default_hints", rename = "languageHints")]
    pub language_hints: Vec<String>,
}

/// Non-secret Volcengine fields. Names match Type4Me's `tf_asr_volcano` object.
/// `appKey`, `accessKey`, and `apiKey` are stripped into Credential Manager.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolcanoSettings {
    #[serde(default = "default_auth_mode", rename = "authMode")]
    pub auth_mode: String,
    #[serde(default = "default_volc_resource", rename = "resourceId")]
    pub resource_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub hotkey: HotkeyConfig,
    #[serde(default)]
    pub injection: InjectionConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub overlay: OverlayConfig,
    /// Only Quick Mode is implemented. Other values are preserved for a future importer.
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_provider", rename = "tf_selectedASRProvider")]
    pub selected_asr_provider: String,
    #[serde(default, rename = "tf_asr_soniox")]
    pub soniox: SonioxSettings,
    #[serde(default, rename = "tf_asr_volcano")]
    pub volcano: VolcanoSettings,
    /// UI language. `en` or a `zh` tag. Mirrors Type4Me's `tf_language` key.
    #[serde(default = "default_language", rename = "tf_language")]
    pub language: String,
    /// Extra Type4Me keys (LLM fields and other providers) kept so a copied
    /// Mac file is not stripped down to the Windows subset.
    #[serde(default, flatten)]
    pub preserved: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedSecret {
    pub account: String,
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadOutcome {
    pub config: Config,
    pub extracted_secrets: Vec<ExtractedSecret>,
    pub stripped_secrets: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("config io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            binding: DEFAULT_BINDING.into(),
            activation: Activation::PushToTalk,
        }
    }
}

impl Default for InjectionConfig {
    fn default() -> Self {
        Self {
            method: InjectionMethod::Auto,
        }
    }
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            show_partials: true,
        }
    }
}

impl Default for VolcanoSettings {
    fn default() -> Self {
        Self {
            auth_mode: default_auth_mode(),
            resource_id: default_volc_resource(),
        }
    }
}

impl Default for SonioxSettings {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.into(),
            endpoint_sensitivity: "-0.3".into(),
            language_hints: vec!["zh".into(), "en".into()],
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            hotkey: HotkeyConfig::default(),
            injection: InjectionConfig::default(),
            audio: AudioConfig::default(),
            overlay: OverlayConfig::default(),
            mode: default_mode(),
            selected_asr_provider: default_provider(),
            soniox: SonioxSettings::default(),
            volcano: VolcanoSettings::default(),
            language: default_language(),
            preserved: Map::new(),
        }
    }
}

fn default_version() -> u32 {
    CONFIG_VERSION
}
fn default_mode() -> String {
    "quick".into()
}
fn default_provider() -> String {
    "volcano".into()
}
fn default_auth_mode() -> String {
    "legacy".into()
}
fn default_volc_resource() -> String {
    DEFAULT_VOLC_RESOURCE.into()
}
fn default_language() -> String {
    "en".into()
}
fn default_model() -> String {
    DEFAULT_MODEL.into()
}
fn default_sensitivity() -> String {
    "-0.3".into()
}
fn default_hints() -> Vec<String> {
    vec!["zh".into(), "en".into()]
}

impl Config {
    pub fn binding(&self) -> Result<Binding, crate::hotkey::HotkeyParseError> {
        Binding::parse(&self.hotkey.binding)
    }

    pub fn endpoint_sensitivity(&self) -> f64 {
        parse_sensitivity(&self.soniox.endpoint_sensitivity).unwrap_or(-0.3)
    }

    pub fn normalize(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();
        if Binding::parse(&self.hotkey.binding).is_err() {
            warnings.push(format!(
                "hotkey `{}` is not valid; using {DEFAULT_BINDING}",
                self.hotkey.binding
            ));
            self.hotkey.binding = DEFAULT_BINDING.into();
        }
        if self.soniox.model.trim().is_empty() {
            self.soniox.model = DEFAULT_MODEL.into();
        } else {
            self.soniox.model = self.soniox.model.trim().to_string();
        }
        let sensitivity = parse_sensitivity(&self.soniox.endpoint_sensitivity).unwrap_or(-0.3);
        let clamped = sensitivity.clamp(-1.0, 1.0);
        if (clamped - sensitivity).abs() > f64::EPSILON {
            warnings.push(format!(
                "endpoint sensitivity {sensitivity} is outside [-1, 1]; clamped"
            ));
        }
        self.soniox.endpoint_sensitivity = format_sensitivity(clamped);
        self.soniox.language_hints = normalize_hints(&self.soniox.language_hints);
        if let Some(device) = &self.audio.input_device {
            let trimmed = device.trim();
            self.audio.input_device = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
        }
        let provider = self.selected_asr_provider.trim().to_string();
        self.selected_asr_provider = match provider.as_str() {
            "" => default_provider(),
            "volcengine" => "volcano".into(),
            other => other.to_string(),
        };
        if self.volcano.resource_id.trim().is_empty() {
            self.volcano.resource_id = default_volc_resource();
        } else {
            self.volcano.resource_id = self.volcano.resource_id.trim().to_string();
        }
        if self.volcano.auth_mode.trim().is_empty() {
            self.volcano.auth_mode = default_auth_mode();
        }
        self.language = if self.language.to_ascii_lowercase().starts_with("zh") {
            "zh-CN".into()
        } else {
            "en".into()
        };
        warnings
    }
}

pub fn config_path(app_data: &Path) -> PathBuf {
    app_data.join("Type4We").join("config.json")
}

pub fn load_from_path(path: &Path) -> Result<LoadOutcome, ConfigError> {
    match fs::read_to_string(path) {
        Ok(text) => load_from_str(&text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(LoadOutcome {
            config: Config::default(),
            extracted_secrets: Vec::new(),
            stripped_secrets: false,
            warnings: Vec::new(),
        }),
        Err(err) => Err(err.into()),
    }
}

pub fn load_from_str(text: &str) -> Result<LoadOutcome, ConfigError> {
    let mut value: Value = serde_json::from_str(text)?;
    let extracted = strip_secrets(&mut value);
    let stripped = !extracted.is_empty();
    let mut config: Config = serde_json::from_value(value)?;
    let warnings = config.normalize();
    Ok(LoadOutcome {
        config,
        extracted_secrets: extracted,
        stripped_secrets: stripped,
        warnings,
    })
}

pub fn save_to_path(path: &Path, config: &Config) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut value = serde_json::to_value(config)?;
    strip_secrets(&mut value);
    let body = serde_json::to_vec_pretty(&value)?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&body)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn parse_sensitivity(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

fn format_sensitivity(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    let text = format!("{rounded}");
    if text == "-0" {
        "0".into()
    } else {
        text
    }
}

fn normalize_hints(hints: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for hint in hints {
        for piece in hint.split([',', ' ']) {
            let trimmed = piece.trim().to_ascii_lowercase();
            if trimmed.is_empty() || out.iter().any(|existing: &String| existing == &trimmed) {
                continue;
            }
            out.push(trimmed);
        }
    }
    out
}

fn strip_secrets(value: &mut Value) -> Vec<ExtractedSecret> {
    let mut found = Vec::new();
    strip_secrets_rec(value, None, &mut found);
    found
}

fn strip_secrets_rec(value: &mut Value, account: Option<&str>, found: &mut Vec<ExtractedSecret>) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let secret_keys: Vec<String> = object
        .keys()
        .filter(|key| SECRET_FIELDS.contains(&key.as_str()))
        .cloned()
        .collect();
    for key in secret_keys {
        if let Some(removed) = object.remove(&key) {
            if let (Some(account), Some(secret)) = (account, removed.as_str()) {
                let secret = secret.trim();
                if !secret.is_empty() {
                    found.push(ExtractedSecret {
                        account: account.to_string(),
                        field: key,
                        value: secret.to_string(),
                    });
                }
            }
        }
    }
    let nested: Vec<(String, Value)> = object
        .iter()
        .filter_map(|(key, child)| child.as_object().map(|_| (key.clone(), child.clone())))
        .collect();
    for (key, mut child) in nested {
        let next_account = if key.starts_with("tf_asr_") || key.starts_with("tf_llm_") {
            Some(key.as_str())
        } else {
            None
        };
        strip_secrets_rec(&mut child, next_account, found);
        object.insert(key, child);
    }
}

fn de_flex_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(text) => Ok(text),
        Value::Number(number) => Ok(number.to_string()),
        other => Err(serde::de::Error::custom(format!(
            "expected string or number, got {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_the_default_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        let outcome = load_from_path(&path).unwrap();
        assert_eq!(outcome.config, Config::default());
        assert!(!outcome.stripped_secrets);
    }

    #[test]
    fn roundtrip_keeps_type4me_provider_block_and_extra_keys() {
        let mut config = Config::default();
        config.hotkey.binding = "Ctrl+Win".into();
        config.hotkey.activation = Activation::Toggle;
        config.soniox.model = "stt-rt-v5".into();
        config.soniox.endpoint_sensitivity = "0.3".into();
        config.volcano.auth_mode = "apiKey".into();
        config.volcano.resource_id = "volc.bigasr.sauc.duration".into();
        config.preserved.insert(
            "tf_llm_doubao".into(),
            serde_json::json!({"model": "doubao-pro"}),
        );
        let dir = tempfile::tempdir().unwrap();
        let path = config_path(dir.path());
        save_to_path(&path, &config).unwrap();
        let loaded = load_from_path(&path).unwrap();
        assert_eq!(loaded.config.hotkey.binding, "Ctrl+Win");
        assert_eq!(loaded.config.hotkey.activation, Activation::Toggle);
        assert_eq!(loaded.config.soniox.model, "stt-rt-v5");
        assert_eq!(loaded.config.volcano.auth_mode, "apiKey");
        assert_eq!(
            loaded.config.volcano.resource_id,
            "volc.bigasr.sauc.duration"
        );
        assert_eq!(
            loaded.config.preserved["tf_llm_doubao"]["model"],
            "doubao-pro"
        );
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("tf_selectedASRProvider"));
        assert!(text.contains("tf_asr_soniox"));
        assert!(text.contains("tf_asr_volcano"));
        assert!(text.contains("volc.bigasr.sauc.duration"));
        assert!(!text.contains("super-secret"));
    }

    #[test]
    fn api_keys_are_extracted_and_not_written_back() {
        let raw = r#"{
            "tf_selectedASRProvider": "soniox",
            "tf_asr_soniox": {
                "apiKey": "  super-secret  ",
                "model": "stt-rt-v6",
                "endpointSensitivity": -0.3
            },
            "tf_asr_volcano": {
                "appKey": "app",
                "accessKey": "token-value",
                "resourceId": "volc.seedasr.sauc.duration"
            }
        }"#;
        let outcome = load_from_str(raw).unwrap();
        assert!(outcome.stripped_secrets);
        assert_eq!(outcome.config.soniox.model, "stt-rt-v6");
        assert_eq!(outcome.config.endpoint_sensitivity(), -0.3);
        assert!(outcome
            .extracted_secrets
            .iter()
            .any(|secret| secret.account == "tf_asr_soniox"
                && secret.field == "apiKey"
                && secret.value == "super-secret"));
        assert!(outcome
            .extracted_secrets
            .iter()
            .any(|secret| secret.account == "tf_asr_volcano"
                && secret.field == "accessKey"
                && secret.value == "token-value"));
        assert!(outcome
            .extracted_secrets
            .iter()
            .any(|secret| secret.account == "tf_asr_volcano"
                && secret.field == "appKey"
                && secret.value == "app"));
        assert_eq!(
            outcome.config.volcano.resource_id,
            "volc.seedasr.sauc.duration"
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        save_to_path(&path, &outcome.config).unwrap();
        let saved = fs::read_to_string(path).unwrap();
        assert!(!saved.contains("super-secret"));
        assert!(!saved.contains("token-value"));
        assert!(saved.contains("volc.seedasr.sauc.duration"));
    }

    #[test]
    fn invalid_binding_falls_back_and_sensitivity_is_clamped() {
        let raw = r#"{
            "hotkey": {"binding": "Ctrl+NotAKey", "activation": "push_to_talk"},
            "tf_asr_soniox": {"model": "", "endpointSensitivity": "9", "languageHints": ["EN", "en", "zh"]}
        }"#;
        let outcome = load_from_str(raw).unwrap();
        assert_eq!(outcome.config.hotkey.binding, "RAlt");
        assert_eq!(outcome.config.soniox.model, DEFAULT_MODEL);
        assert_eq!(outcome.config.soniox.endpoint_sensitivity, "1");
        assert_eq!(outcome.config.soniox.language_hints, vec!["en", "zh"]);
        assert!(!outcome.warnings.is_empty());
    }

    #[test]
    fn invalid_json_is_an_error() {
        let err = load_from_str("{").unwrap_err();
        assert!(matches!(err, ConfigError::Json(_)));
    }

    #[test]
    fn example_config_parses() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config.example.json");
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        let outcome = load_from_str(&text).unwrap();
        assert_eq!(outcome.config.selected_asr_provider, "volcano");
        assert_eq!(
            outcome.config.volcano.resource_id,
            "volc.seedasr.sauc.duration"
        );
        assert!(outcome.extracted_secrets.is_empty());
        assert!(outcome.config.binding().is_ok());
    }
}
