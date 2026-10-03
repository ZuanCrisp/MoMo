// API keys live in the Windows Credential Manager or, on Linux, the Secret
// Service (GNOME Keyring, KWallet) — never on disk and never in the front end — the island can only ask whether a key is present.

use keyring::Entry;

const SERVICE: &str = "app.momo.desktop";

fn ai_entry(account: &str) -> Result<Entry, String> {
    let mut parts = account.split(':');
    let valid = account == "anthropic-api-key"
        || (parts.next() == Some("ai")
            && parts.next().is_some_and(crate::ai::config::valid_id)
            && parts.next().is_some_and(crate::ai::config::valid_id)
            && parts.next().is_none());
    if !valid {
        return Err("Invalid AI credential account.".into());
    }
    Entry::new(SERVICE, account).map_err(|_| "Cannot open the system credential store.".into())
}

pub(crate) fn read_ai(account: &str) -> Result<Option<String>, String> {
    match ai_entry(account)?.get_password() {
        Ok(value) => Ok(Some(value).filter(|value| !value.is_empty())),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("Cannot read the system credential store. Unlock it and try again.".into()),
    }
}

pub(crate) fn write_ai(account: &str, value: Option<&str>) -> Result<(), String> {
    let entry = ai_entry(account)?;
    let result = if let Some(value) = value {
        entry.set_password(value)
    } else {
        entry.delete_credential()
    };
    match result {
        Ok(()) | Err(keyring::Error::NoEntry) if value.is_none() => Ok(()),
        Ok(()) => Ok(()),
        Err(_) => Err("Cannot update the system credential store.".into()),
    }
}

/// Every key MoMo may store. Anything outside this list is refused.
pub const KNOWN_KEYS: &[&str] = &[
    "anthropic-api-key",
    "n8n-url",
    "n8n-api-key",
    "vercel-token",
    "github-token",
    "stripe-api-key",
    "resend-api-key",
    "notion-api-key",
    "calcom-api-key",
];

fn entry(key: &str) -> Option<Entry> {
    if !KNOWN_KEYS.contains(&key) {
        return None;
    }
    Entry::new(SERVICE, key).ok()
}

pub fn get(key: &str) -> Option<String> {
    entry(key)?.get_password().ok().filter(|v| !v.is_empty())
}

pub fn set(key: &str, value: &str) -> Result<(), String> {
    let entry = entry(key).ok_or_else(|| format!("unknown key {key}"))?;
    if value.is_empty() {
        let _ = entry.delete_credential();
        return Ok(());
    }
    entry.set_password(value).map_err(|e| e.to_string())
}

pub fn clear(key: &str) -> Result<(), String> {
    let entry = entry(key).ok_or_else(|| format!("unknown key {key}"))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn present(key: &str) -> bool {
    get(key).is_some()
}
