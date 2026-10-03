mod chat;
pub mod config;
mod providers;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{secrets, settings::Settings};
pub use chat::{send, Chat, ChatContext, ChatReply};
pub use config::{Config, Profile};
pub use providers::Model;

/// Secret values exist only in the incoming command and credential store.
/// This type must never be serialized into settings or logged.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyUpdate {
    pub profile_id: String,
    pub key_id: String,
    pub value: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStatus {
    pub profile_id: String,
    pub key_id: String,
    pub present: bool,
}

pub fn key_status(config: &Config) -> Result<Vec<KeyStatus>, String> {
    let mut statuses = Vec::new();
    for profile in &config.profiles {
        for slot in &profile.keys {
            statuses.push(KeyStatus {
                profile_id: profile.id.clone(),
                key_id: slot.id.clone(),
                present: secrets::read_ai(&profile.account(slot))?.is_some(),
            });
        }
    }
    Ok(statuses)
}

pub fn validate_key(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 4096 || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err("Paste one API key per row, without spaces or line breaks.".into());
    }
    Ok(value.into())
}

pub trait Vault {
    fn read(&self, account: &str) -> Result<Option<String>, String>;
    fn write(&self, account: &str, value: Option<&str>) -> Result<(), String>;
}

pub struct SystemVault;
impl Vault for SystemVault {
    fn read(&self, account: &str) -> Result<Option<String>, String> {
        secrets::read_ai(account)
    }
    fn write(&self, account: &str, value: Option<&str>) -> Result<(), String> {
        secrets::write_ai(account, value)
    }
}

struct Change {
    account: String,
    previous: Option<String>,
    next: Option<String>,
}

/// Validate first, then update the vault and metadata as one operation. A failed
/// credential/settings write restores previous credentials; no key is put in JSON.
pub fn save_config<V, F>(
    current: &mut Settings,
    mut config: Config,
    updates: Vec<KeyUpdate>,
    vault: &V,
    persist: F,
) -> Result<Settings, String>
where
    V: Vault,
    F: FnOnce(&Settings) -> Result<(), String>,
{
    config.validate()?;
    let mut replacements = HashMap::new();
    for update in updates {
        let profile = config
            .profiles
            .iter()
            .find(|p| p.id == update.profile_id)
            .ok_or("API key refers to an unknown profile.")?;
        let slot = profile
            .keys
            .iter()
            .find(|k| k.id == update.key_id)
            .ok_or("API key refers to an unknown key slot.")?;
        if replacements
            .insert(profile.account(slot), validate_key(&update.value)?)
            .is_some()
        {
            return Err("An API key row was submitted twice.".into());
        }
    }
    let old_accounts: HashSet<_> = current
        .ai
        .profiles
        .iter()
        .flat_map(|p| p.keys.iter().map(|k| p.account(k)))
        .collect();
    let new_accounts: HashSet<_> = config
        .profiles
        .iter()
        .flat_map(|p| p.keys.iter().map(|k| p.account(k)))
        .collect();
    for profile in &config.profiles {
        let previous = current.ai.profiles.iter().find(|p| p.id == profile.id);
        let changed_server = previous.is_some_and(|old| {
            old.provider != profile.provider || old.base_url != profile.base_url
        });
        for slot in &profile.keys {
            let account = profile.account(slot);
            if (!old_accounts.contains(&account) || changed_server)
                && !replacements.contains_key(&account)
            {
                return Err("Paste new keys or remove the empty rows. Changing a provider/server requires replacing its saved keys.".into());
            }
        }
    }
    let mut changes = Vec::new();
    for (account, next) in replacements {
        changes.push(Change {
            previous: vault.read(&account)?,
            account,
            next: Some(next),
        });
    }
    for account in old_accounts.difference(&new_accounts) {
        changes.push(Change {
            account: account.clone(),
            previous: vault.read(account)?,
            next: None,
        });
    }
    let mut next = current.clone();
    next.ai = config;
    let restore = |changes: &[Change]| {
        let mut failed = false;
        for change in changes.iter().rev() {
            if vault
                .write(&change.account, change.previous.as_deref())
                .is_err()
            {
                failed = true;
            }
        }
        failed
    };
    for (index, change) in changes.iter().enumerate() {
        if vault
            .write(&change.account, change.next.as_deref())
            .is_err()
        {
            let incomplete = restore(&changes[..=index]);
            return Err(if incomplete { "Credential update failed and could not be fully restored. Check your saved keys before retrying." } else { "Could not save credentials. Your previous settings and keys were restored." }.into());
        }
    }
    if persist(&next).is_err() {
        let incomplete = restore(&changes);
        return Err(if incomplete { "Settings could not be saved and some credentials could not be restored. Check your saved keys before retrying." } else { "Could not save settings. Your previous keys were restored." }.into());
    }
    *current = next.clone();
    Ok(next)
}

/// The UI can discover models before saving a new profile, using a pasted key.
/// Saved credentials are usable only for the same registered server/provider.
pub async fn list_models(
    mut profile: Profile,
    draft_key: Option<String>,
    saved: Config,
) -> Result<Vec<Model>, String> {
    profile.validate()?;
    let mut keys = Vec::new();
    if let Some(value) = draft_key.filter(|value| !value.trim().is_empty()) {
        keys.push(Some(validate_key(&value)?));
    } else if let Some(old) = saved.profiles.iter().find(|old| {
        old.id == profile.id && old.provider == profile.provider && old.base_url == profile.base_url
    }) {
        for slot in &profile.keys {
            if old.keys.iter().any(|key| key.id == slot.id) {
                if let Some(value) = secrets::read_ai(&old.account(slot))? {
                    keys.push(Some(value));
                }
            }
        }
    }
    if keys.is_empty() {
        if profile.provider.is_local() && profile.keys.is_empty() {
            keys.push(None);
        } else {
            return Err(
                "Paste an API key first, then load models. You can also enter a model ID manually."
                    .into(),
            );
        }
    }
    let request = async {
        let mut error = "Could not load models.".to_string();
        for key in keys {
            match providers::models(&profile, key.as_deref()).await {
                Ok(models) => return Ok(models),
                Err(failure) => {
                    error = failure.message;
                    if failure.retry != providers::Retry::Key {
                        break;
                    }
                }
            }
        }
        Err(error)
    };
    tokio::time::timeout(std::time::Duration::from_secs(40), request)
        .await
        .map_err(|_| "Loading models timed out. Check your server and try again.")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct MemoryVault {
        values: RefCell<HashMap<String, String>>,
    }
    impl Vault for MemoryVault {
        fn read(&self, account: &str) -> Result<Option<String>, String> {
            Ok(self.values.borrow().get(account).cloned())
        }
        fn write(&self, account: &str, value: Option<&str>) -> Result<(), String> {
            if let Some(value) = value {
                self.values
                    .borrow_mut()
                    .insert(account.into(), value.into());
            } else {
                self.values.borrow_mut().remove(account);
            }
            Ok(())
        }
    }

    fn replacement() -> KeyUpdate {
        KeyUpdate {
            profile_id: "legacy-claude".into(),
            key_id: "legacy".into(),
            value: "fake-new-value".into(),
        }
    }

    #[test]
    fn failed_metadata_write_restores_the_vault_and_current_settings() {
        let mut settings = Settings::default();
        let original = settings.ai.clone();
        let vault = MemoryVault::default();
        vault
            .write("anthropic-api-key", Some("fake-original"))
            .unwrap();
        let error = save_config(
            &mut settings,
            original.clone(),
            vec![replacement()],
            &vault,
            |_| Err("write failure".into()),
        )
        .unwrap_err();
        assert!(error.contains("restored"));
        assert_eq!(settings.ai, original);
        assert_eq!(
            vault.read("anthropic-api-key").unwrap().as_deref(),
            Some("fake-original")
        );
    }

    #[test]
    fn saved_metadata_never_contains_secret_values_and_removal_clears_credentials() {
        let mut settings = Settings::default();
        let vault = MemoryVault::default();
        let config = settings.ai.clone();
        save_config(
            &mut settings,
            config,
            vec![replacement()],
            &vault,
            |saved| {
                let json = serde_json::to_string(saved).unwrap();
                assert!(!json.contains("fake-new-value"));
                Ok(())
            },
        )
        .unwrap();
        let empty = Config {
            profiles: vec![],
            active_profile_id: String::new(),
            fallback_enabled: false,
            fallback_profile_ids: vec![],
        };
        save_config(&mut settings, empty, vec![], &vault, |_| Ok(())).unwrap();
        assert!(vault.read("anthropic-api-key").unwrap().is_none());
    }

    #[test]
    fn provider_changes_cannot_reuse_credentials_at_a_new_endpoint() {
        let mut settings = Settings::default();
        let mut config = settings.ai.clone();
        let p = &mut config.profiles[0];
        p.provider = config::Provider::OpenaiCompatible;
        p.base_url = "https://example.com/v1".into();
        p.web_search = false;
        p.keys[0].id = "other".into();
        assert!(save_config(
            &mut settings,
            config,
            vec![],
            &MemoryVault::default(),
            |_| Ok(())
        )
        .is_err());
        assert!(validate_key("two keys\non different lines").is_err());
    }
}
