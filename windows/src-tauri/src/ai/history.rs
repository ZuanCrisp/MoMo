//! Private, bounded conversation archives. Reopening only restores text context;
//! old desktop actions are receipts and are never executed again.
use super::chat::ChatReply;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_CHATS: usize = 100;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: usize,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<ChatReply>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    pub messages: Vec<Entry>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    pub message_count: usize,
}

pub struct History {
    path: PathBuf,
    chats: Vec<Conversation>,
    pub active: Option<String>,
    error: Option<String>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn clipped(text: &str) -> String {
    if text.len() <= 32768 {
        return text.to_owned();
    }
    let mut end = 32768;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[Long text shortened in saved history.]", &text[..end])
}
impl Default for History {
    fn default() -> Self {
        #[cfg(not(test))]
        let path = crate::settings::local_dir().join("chat-history.json");
        #[cfg(test)]
        let path = std::env::temp_dir().join(format!(
            "momo-chat-test-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        Self::load(path)
    }
}
impl History {
    fn load(path: PathBuf) -> Self {
        let result = match std::fs::metadata(&path) {
            Ok(meta) if meta.len() > MAX_BYTES as u64 => {
                Err("Saved history exceeds the storage limit.".into())
            }
            Ok(_) => std::fs::read(&path)
                .map_err(|_| "Cannot read saved history.".to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<Vec<Conversation>>(&bytes).map_err(|_| {
                        "Saved history is damaged; the original file was preserved.".into()
                    })
                }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(_) => Err("Cannot access saved history.".into()),
        };
        let (chats, error) = match result {
            Ok(chats)
                if chats.len() <= MAX_CHATS
                    && chats.iter().all(|c| {
                        super::config::valid_id(&c.id)
                            && c.messages.len() <= 200
                            && c.messages
                                .iter()
                                .all(|m| matches!(m.role.as_str(), "user" | "assistant"))
                    }) =>
            {
                (chats, None)
            }
            Ok(_) => (
                vec![],
                Some("Saved history has invalid entries; the original file was preserved.".into()),
            ),
            Err(error) => (vec![], Some(error)),
        };
        Self {
            path,
            chats,
            active: None,
            error,
        }
    }
    pub fn list(&self) -> Result<Vec<Summary>, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let mut chats: Vec<_> = self
            .chats
            .iter()
            .map(|c| Summary {
                id: c.id.clone(),
                title: c.title.clone(),
                updated_at: c.updated_at,
                message_count: c.messages.len(),
            })
            .collect();
        chats.sort_by_key(|c| std::cmp::Reverse(c.updated_at));
        Ok(chats)
    }
    pub fn open(&mut self, id: &str) -> Result<Conversation, String> {
        self.list()?;
        let chat = self
            .chats
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or("This conversation is no longer available.")?;
        self.active = Some(id.into());
        Ok(chat)
    }
    pub fn record(&mut self, query: &str, reply: &ChatReply) -> Result<(), String> {
        self.list()?;
        let before = self.chats.clone();
        let active_before = self.active.clone();
        let id = self.active.clone().unwrap_or_else(|| {
            format!(
                "chat-{:x}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            )
        });
        if !self.chats.iter().any(|c| c.id == id) {
            self.chats.push(Conversation {
                id: id.clone(),
                title: query.chars().take(70).collect(),
                updated_at: now(),
                messages: vec![],
            });
        }
        self.active = Some(id.clone());
        let chat = self.chats.iter_mut().find(|c| c.id == id).unwrap();
        let next = chat.messages.last().map_or(1, |m| m.id + 1);
        let mut route = reply.clone();
        route.text.clear();
        chat.messages.push(Entry {
            id: next,
            role: "user".into(),
            content: clipped(query),
            route: None,
        });
        chat.messages.push(Entry {
            id: next + 1,
            role: "assistant".into(),
            content: clipped(&reply.text),
            route: Some(route),
        });
        if chat.messages.len() > 200 {
            chat.messages.drain(..2);
        }
        chat.updated_at = now();
        self.chats.sort_by_key(|c| std::cmp::Reverse(c.updated_at));
        self.chats.truncate(MAX_CHATS);
        while serde_json::to_vec(&self.chats)
            .map_err(|_| "Cannot encode history.")?
            .len()
            > MAX_BYTES
        {
            if self.chats.len() <= 1 {
                self.chats[0].messages.drain(..2);
            } else {
                let index = self.chats.iter().rposition(|c| c.id != id).unwrap();
                self.chats.remove(index);
            }
        }
        if let Err(error) = self.save() {
            self.chats = before;
            self.active = active_before;
            return Err(error);
        }
        Ok(())
    }
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        self.list()?;
        let before = self.chats.clone();
        self.chats.retain(|c| c.id != id);
        if let Err(error) = self.save() {
            self.chats = before;
            return Err(error);
        }
        if self.active.as_deref() == Some(id) {
            self.active = None;
        }
        Ok(())
    }
    fn save(&self) -> Result<(), String> {
        let parent = self.path.parent().ok_or("Invalid history folder.")?;
        crate::platform::ensure_private_dir(parent).map_err(|_| "Cannot create history folder.")?;
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(
            &temp,
            serde_json::to_vec(&self.chats).map_err(|_| "Cannot encode history.")?,
        )
        .map_err(|_| "Cannot save chat history.")?;
        std::fs::rename(temp, &self.path).map_err(|_| "Cannot update chat history.".to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_round_trip_restores_receipts_and_delete_persists() {
        let path = std::env::temp_dir().join(format!("momo-history-{}.json", now()));
        let mut history = History::load(path.clone());
        let reply = ChatReply {
            text: "Saved note".into(),
            profile_id: "test".into(),
            profile_name: "Local".into(),
            model: "model".into(),
            used_fallback: false,
            key_label: None,
            actions: vec![],
            history_saved: true,
            conversation_id: None,
        };
        history.record("Create a note", &reply).unwrap();
        let id = history.list().unwrap()[0].id.clone();
        let mut loaded = History::load(path.clone());
        assert_eq!(loaded.open(&id).unwrap().messages[1].content, "Saved note");
        loaded.record("Continue", &reply).unwrap();
        assert_eq!(loaded.list().unwrap()[0].message_count, 4);
        loaded.delete(&id).unwrap();
        assert!(History::load(path.clone()).list().unwrap().is_empty());
        std::fs::write(&path, b"invalid").unwrap();
        let mut damaged = History::load(path.clone());
        assert!(damaged.record("new", &reply).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid");
        std::fs::remove_file(path).unwrap();
    }
}
