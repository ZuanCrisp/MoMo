use std::collections::HashSet;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

pub const LEGACY_MODEL: &str = "claude-opus-5";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    Anthropic,
    Openai,
    Gemini,
    OpenaiCompatible,
    Ollama,
    LocalOpenai,
}

impl Provider {
    pub fn is_local(self) -> bool {
        matches!(self, Self::Ollama | Self::LocalOpenai)
    }

    pub fn default_url(self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Openai => "https://api.openai.com/v1",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta",
            Self::Ollama => "http://127.0.0.1:11434",
            Self::LocalOpenai => "http://127.0.0.1:1234/v1",
            Self::OpenaiCompatible => "",
        }
    }

    pub fn supports_search(self) -> bool {
        matches!(self, Self::Anthropic | Self::Openai | Self::Gemini)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeySlot {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub provider: Provider,
    pub base_url: String,
    pub model: String,
    pub keys: Vec<KeySlot>,
    #[serde(default = "default_tokens")]
    pub max_output_tokens: u32,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub web_search: bool,
}

fn default_tokens() -> u32 {
    4096
}
fn default_timeout() -> u64 {
    120
}

impl Profile {
    pub fn account(&self, slot: &KeySlot) -> String {
        if self.id == "legacy-claude" && slot.id == "legacy" {
            "anthropic-api-key".into()
        } else {
            format!("ai:{}:{}", self.id, slot.id)
        }
    }

    pub fn validate(&mut self) -> Result<(), String> {
        if !valid_id(&self.id) {
            return Err("Invalid profile ID.".into());
        }
        self.name = self.name.trim().into();
        self.model = self.model.trim().into();
        if self.name.is_empty()
            || self.name.chars().count() > 80
            || self.name.chars().any(char::is_control)
        {
            return Err("Give each profile a name of 1–80 characters.".into());
        }
        if self.model.len() > 200
            || self
                .model
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(
                "Use a model ID without spaces, such as the ID returned by Load models.".into(),
            );
        }
        if self.provider == Provider::Gemini {
            self.model = self.model.trim_start_matches("models/").into();
            if !self
                .model
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
            {
                return Err("Use the Gemini model ID returned by Load models.".into());
            }
        }
        if self.provider == Provider::Ollama && self.model.to_ascii_lowercase().contains("cloud") {
            return Err(
                "Choose a downloaded Ollama model for a local profile, not a cloud model.".into(),
            );
        }
        self.base_url = self.base_url.trim().trim_end_matches('/').into();
        let url = reqwest::Url::parse(&self.base_url).map_err(|_| {
            "Enter a complete server URL, including http:// or https://.".to_string()
        })?;
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("Keep API keys out of the URL. Paste them into the API key fields.".into());
        }
        let host = url.host_str().unwrap_or("").trim_matches(['[', ']']);
        let loopback = host.eq_ignore_ascii_case("localhost")
            || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
        if self.provider.is_local() && !loopback {
            return Err(
                "Local profiles must connect to localhost or a loopback address on this device."
                    .into(),
            );
        }
        if !matches!(url.scheme(), "http" | "https") || (url.scheme() == "http" && !loopback) {
            return Err(
                "Use HTTPS for an online server. HTTP is allowed only on this device.".into(),
            );
        }
        if !self.provider.default_url().is_empty()
            && !self.provider.is_local()
            && self.base_url != self.provider.default_url()
        {
            return Err(
                "Use the official provider URL, or choose OpenAI-compatible for a custom server."
                    .into(),
            );
        }
        if !(16..=65536).contains(&self.max_output_tokens)
            || !(10..=600).contains(&self.timeout_seconds)
        {
            return Err(
                "Output limit must be 16–65536 tokens; timeout must be 10–600 seconds.".into(),
            );
        }
        if self.web_search && !self.provider.supports_search() {
            return Err(
                "Web search is available only for Claude, OpenAI and Gemini profiles.".into(),
            );
        }
        if self.keys.len() > 16 {
            return Err("A profile can store up to 16 API keys.".into());
        }
        let mut ids = HashSet::new();
        for slot in &mut self.keys {
            slot.label = slot.label.trim().into();
            if !valid_id(&slot.id)
                || !ids.insert(&slot.id)
                || slot.label.is_empty()
                || slot.label.chars().count() > 60
                || slot.label.chars().any(char::is_control)
            {
                return Err(
                    "Give every API key a unique ID and a label of 1–60 characters.".into(),
                );
            }
            if slot.id == "legacy"
                && !(self.id == "legacy-claude"
                    && slot.id == "legacy"
                    && self.provider == Provider::Anthropic)
            {
                return Err("The imported key belongs to the original Claude profile.".into());
            }
        }
        Ok(())
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub profiles: Vec<Profile>,
    pub active_profile_id: String,
    pub fallback_enabled: bool,
    pub fallback_profile_ids: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self::legacy(LEGACY_MODEL)
    }
}

impl Config {
    pub fn legacy(model: &str) -> Self {
        Self {
            profiles: vec![Profile {
                id: "legacy-claude".into(),
                name: "Claude".into(),
                provider: Provider::Anthropic,
                base_url: Provider::Anthropic.default_url().into(),
                model: model.into(),
                keys: vec![KeySlot {
                    id: "legacy".into(),
                    label: "Main key".into(),
                }],
                max_output_tokens: default_tokens(),
                timeout_seconds: default_timeout(),
                web_search: true,
            }],
            active_profile_id: "legacy-claude".into(),
            fallback_enabled: false,
            fallback_profile_ids: vec![],
        }
    }

    pub fn validate(&mut self) -> Result<(), String> {
        if self.profiles.len() > 32 {
            return Err("You can save up to 32 model profiles.".into());
        }
        let mut ids = HashSet::new();
        for profile in &mut self.profiles {
            profile.validate()?;
            if !ids.insert(profile.id.clone()) {
                return Err("Profile IDs must be unique.".into());
            }
        }
        if !(self.active_profile_id.is_empty() && self.profiles.is_empty())
            && !ids.contains(&self.active_profile_id)
        {
            return Err("Choose an existing profile as the active model.".into());
        }
        let mut fallbacks = HashSet::new();
        for id in &self.fallback_profile_ids {
            if !ids.contains(id) || id == &self.active_profile_id || !fallbacks.insert(id) {
                return Err(
                    "Fallback profiles must be unique, saved profiles other than the active one."
                        .into(),
                );
            }
        }
        Ok(())
    }

    pub fn routes(&self) -> Vec<&Profile> {
        let mut ids = vec![&self.active_profile_id];
        if self.fallback_enabled {
            ids.extend(self.fallback_profile_ids.iter());
        }
        ids.into_iter()
            .filter_map(|id| self.profiles.iter().find(|p| &p.id == id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_routes_do_not_include_cloud_without_explicit_fallback() {
        let mut config = Config::default();
        let mut local = config.profiles[0].clone();
        local.id = "local".into();
        local.provider = Provider::Ollama;
        local.keys.clear();
        local.base_url = Provider::Ollama.default_url().into();
        local.model = "llama3.2".into();
        local.web_search = false;
        config.profiles.push(local);
        config.active_profile_id = "local".into();
        config.fallback_profile_ids.push("legacy-claude".into());
        config.validate().unwrap();
        assert_eq!(config.routes().len(), 1);
        config.fallback_enabled = true;
        assert_eq!(config.routes().len(), 2);
    }

    #[test]
    fn endpoint_validation_prevents_credentials_in_urls_and_remote_local_profiles() {
        let mut p = Config::default().profiles.remove(0);
        p.provider = Provider::OpenaiCompatible;
        p.keys.clear();
        p.web_search = false;
        for url in [
            "https://user:secret@example.com/v1",
            "https://example.com/v1?key=secret",
            "http://example.com/v1",
            "file:///etc/passwd",
        ] {
            p.base_url = url.into();
            assert!(p.validate().is_err());
        }
        p.provider = Provider::LocalOpenai;
        p.base_url = "https://example.com/v1".into();
        assert!(p.validate().is_err());
        p.base_url = "http://[::1]:1234/v1".into();
        assert!(p.validate().is_ok());
    }

    #[test]
    fn configuration_rejects_secrets_as_metadata_and_invalid_routes() {
        let mut value = serde_json::to_value(Config::default()).unwrap();
        value["profiles"][0]["keys"][0]["value"] = "not-a-real-key".into();
        assert!(serde_json::from_value::<Config>(value).is_err());
        let mut config = Config::default();
        config.fallback_profile_ids.push("legacy-claude".into());
        assert!(config.validate().is_err());
    }
}
