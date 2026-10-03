// Preferences, stored as plain JSON in settings.json under platform::config_dir().
// No secret ever lands here — API keys live in the OS keychain (see secrets.rs).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub sound_enabled: bool,
    pub sound_volume: f64,
    pub auto_close_interval: f64,
    pub absence_interval: f64,
    pub active_integrations: Vec<String>,
    /// "primary" = the main display, "cursor" = whichever display the mouse is on.
    pub screen: String,
    pub autostart: bool,
    pub hooks_installed: bool,
    /// Legacy Claude model retained to migrate preferences from older builds.
    /// Defaulted explicitly so a settings.json written by an older build still loads.
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub ai: crate::ai::Config,
    #[serde(default)]
    pub computer_control: bool,
    #[serde(default)]
    pub local_models_directory: String,
}

fn default_model() -> String {
    crate::ai::config::LEGACY_MODEL.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            sound_volume: 0.12,
            auto_close_interval: 15.0,
            absence_interval: 180.0,
            active_integrations: vec![
                "integration_resend".into(),
                "integration_n8n".into(),
                "integration_vercel".into(),
                "integration_github".into(),
            ],
            screen: "primary".into(),
            autostart: false,
            hooks_installed: false,
            model: default_model(),
            ai: crate::ai::Config::default(),
            computer_control: false,
            local_models_directory: String::new(),
        }
    }
}

pub use crate::platform::{config_dir, local_dir};

pub fn hook_exe_path() -> PathBuf {
    local_dir().join("bin").join(crate::platform::HOOK_EXE)
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn load() -> Settings {
    match std::fs::read(settings_path()) {
        Ok(bytes) => decode(&bytes).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

fn decode(bytes: &[u8]) -> Result<Settings, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let legacy = value.get("ai").is_none();
    let mut settings: Settings = serde_json::from_value(value)?;
    if legacy {
        settings.ai = crate::ai::Config::legacy(&settings.model);
    }
    Ok(settings)
}

pub fn save(settings: &Settings) -> std::io::Result<()> {
    let dir = config_dir();
    crate::platform::ensure_private_dir(&dir)?;
    let json = serde_json::to_vec_pretty(settings)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let temporary = dir.join("settings.json.tmp");
    std::fs::write(&temporary, json)?;
    std::fs::rename(temporary, settings_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_preferences_keep_the_selected_claude_model_and_original_key_account() {
        let mut value = serde_json::to_value(Settings::default()).unwrap();
        value.as_object_mut().unwrap().remove("ai");
        value["model"] = "custom-claude-model".into();
        let loaded = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        let profile = &loaded.ai.profiles[0];
        assert_eq!(profile.model, "custom-claude-model");
        assert_eq!(profile.account(&profile.keys[0]), "anthropic-api-key");
        assert!(!loaded.ai.fallback_enabled);
    }
}
