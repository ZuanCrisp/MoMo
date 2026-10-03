use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use super::config::Config;
use super::providers::{self, Retry};

#[derive(Clone)]
pub enum Part {
    Text(String),
    File {
        mime: String,
        data: String,
        name: String,
    },
}

#[derive(Clone)]
pub struct Message {
    pub role: &'static str,
    pub parts: Vec<Part>,
}

#[derive(Default)]
pub struct Chat {
    messages: Mutex<Vec<Message>>,
    turn: tokio::sync::Mutex<()>,
    generation: AtomicU64,
    cancelled: Notify,
}

impl Chat {
    pub fn cancel(&self) {
        let _history = self.messages.lock().unwrap();
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.cancelled.notify_waiters();
    }

    pub fn reset(&self) {
        let mut history = self.messages.lock().unwrap();
        history.clear();
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.cancelled.notify_waiters();
    }
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChatContext {
    File {
        name: String,
        path: String,
    },
    Window {
        app_name: String,
        title: String,
        url: Option<String>,
    },
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatReply {
    pub text: String,
    pub profile_id: String,
    pub profile_name: String,
    pub model: String,
    pub used_fallback: bool,
    pub key_label: Option<String>,
}

pub async fn send<F>(
    chat: &Chat,
    config: &Config,
    query: String,
    context: Option<ChatContext>,
    read_key: F,
) -> Result<ChatReply, String>
where
    F: Fn(&str) -> Result<Option<String>, String> + Sync,
{
    let _turn = chat
        .turn
        .try_lock()
        .map_err(|_| "A reply is already in progress. Stop it before sending another message.")?;
    let cancelled = chat.cancelled.notified();
    tokio::pin!(cancelled);
    cancelled.as_mut().enable();
    let generation = chat.generation.load(Ordering::SeqCst);
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 32000 {
        return Err("Enter a message of 1–32000 characters.".into());
    }
    let mut messages = chat.messages.lock().unwrap().clone();
    if messages.len() >= 200 {
        return Err("This conversation is full. Start a new chat to continue.".into());
    }
    let mut parts = Vec::new();
    if messages.is_empty() {
        match context {
            Some(ChatContext::File { name, path }) => {
                let canonical = std::fs::canonicalize(&path)
                    .map_err(|_| "The attached file is no longer available.")?;
                let inbox = std::fs::canonicalize(crate::files::inbox_dir())
                    .map_err(|_| "The attachment inbox is unavailable.")?;
                if !canonical.starts_with(inbox) {
                    return Err("Choose a file by dropping it into MoMo first.".into());
                }
                parts.push(read_file(&canonical, &name)?);
                parts.push(Part::Text(format!("Attached file: {name}")));
            }
            Some(ChatContext::Window {
                app_name,
                title,
                url,
            }) => {
                let suffix = url.map(|url| format!(", URL: {url}")).unwrap_or_default();
                parts.push(Part::Text(format!(
                    "Context — App: {app_name}, Window: {title}{suffix}"
                )));
            }
            None => {}
        }
    }
    parts.push(Part::Text(query.into()));
    let user = Message {
        role: "user",
        parts,
    };
    messages.push(user.clone());
    let reply = tokio::select! {
        _ = &mut cancelled => return Err("Request cancelled.".into()),
        result = tokio::time::timeout(Duration::from_secs(900), run_routes(config, &messages, read_key)) => {
            result.map_err(|_| "The fallback chain took too long. Check the selected profiles or shorten their timeouts.")??
        }
    };
    let mut history = chat.messages.lock().unwrap();
    if chat.generation.load(Ordering::SeqCst) != generation {
        return Err("Request cancelled.".into());
    }
    // Commit exactly one turn, only after a successful reply. Retries and failed
    // requests never duplicate user messages or corrupt the conversation.
    history.push(user);
    history.push(Message {
        role: "assistant",
        parts: vec![Part::Text(reply.text.clone())],
    });
    Ok(reply)
}

async fn run_routes<F>(
    config: &Config,
    messages: &[Message],
    read_key: F,
) -> Result<ChatReply, String>
where
    F: Fn(&str) -> Result<Option<String>, String> + Sync,
{
    let mut config = config.clone();
    config.validate()?;
    let mut errors = Vec::new();
    for (profile_index, profile) in config.routes().into_iter().enumerate() {
        if profile.model.is_empty() {
            errors.push(format!("{}: choose a model in AI & models.", profile.name));
            continue;
        }
        // Capability failures apply to the whole profile, not to its keys.
        if let Err(error) = providers::body(profile, messages) {
            if error.retry == Retry::Stop {
                return Err(error.message);
            }
            errors.push(format!("{}: {}", profile.name, error.message));
            continue;
        }
        let mut keys = Vec::new();
        if profile.keys.is_empty() && profile.provider.is_local() {
            keys.push((None, None, false));
        }
        for (index, slot) in profile.keys.iter().enumerate() {
            if let Some(value) = read_key(&profile.account(slot))? {
                keys.push((Some(value), Some(slot.label.clone()), index > 0));
            }
        }
        if keys.is_empty() {
            errors.push(format!("{}: add an API key in AI & models.", profile.name));
            continue;
        }
        for (key, label, fallback_key) in keys {
            match providers::complete(profile, key.as_deref(), messages).await {
                Ok(text) => {
                    return Ok(ChatReply {
                        text,
                        profile_id: profile.id.clone(),
                        profile_name: profile.name.clone(),
                        model: profile.model.clone(),
                        used_fallback: profile_index > 0 || fallback_key,
                        key_label: label,
                    })
                }
                Err(error) => {
                    let message = format!("{}: {}", profile.name, error.message);
                    if error.retry == Retry::Stop {
                        return Err(message);
                    }
                    errors.push(message);
                    if error.retry == Retry::Profile {
                        break;
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        return Err("Add an AI profile in Settings → AI & models.".into());
    }
    errors.dedup();
    Err(format!(
        "No configured AI route succeeded.\n{}",
        errors.into_iter().take(6).collect::<Vec<_>>().join("\n")
    ))
}

fn read_file(path: &std::path::Path, name: &str) -> Result<Part, String> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "pdf" => Some("application/pdf"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    };
    let size = std::fs::metadata(path)
        .map_err(|_| "Cannot read the attached file.")?
        .len();
    if let Some(mime) = mime {
        if size > 16 * 1024 * 1024 {
            return Err("Image and PDF attachments must be under 16 MiB.".into());
        }
        let bytes = std::fs::read(path).map_err(|_| "Cannot read the attached file.")?;
        return Ok(Part::File {
            mime: mime.into(),
            data: crate::encoding::base64_for(&bytes),
            name: name.into(),
        });
    }
    if size > 200_000 {
        return Err("Text attachments must be under 200 KB. Choose a smaller file.".into());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|_| "This file is not UTF-8 text, an image or a PDF.")?;
    Ok(Part::Text(format!("File contents:\n{text}")))
}

#[cfg(test)]
mod tests {
    use super::super::config::{KeySlot, Provider};
    use super::*;
    use std::io::{Read, Write};
    use std::sync::Arc;

    fn server(
        responses: Vec<(u16, String)>,
    ) -> (
        String,
        std::sync::mpsc::Receiver<String>,
        std::thread::JoinHandle<()>,
    ) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            for (status, body) in responses {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "Expected request did not reach the mock server"
                            );
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("Mock server failed: {error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 8192];
                loop {
                    let length = stream.read(&mut buffer).unwrap();
                    if length == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..length]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|value| value.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let _ = tx.send(String::from_utf8(bytes).unwrap());
                write!(stream,"HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        (url, rx, thread)
    }

    fn config(url: String) -> Config {
        let mut c = Config::default();
        let p = &mut c.profiles[0];
        p.id = "main".into();
        p.name = "Primary".into();
        p.provider = Provider::OpenaiCompatible;
        p.base_url = url;
        p.model = "model-one".into();
        p.web_search = false;
        p.keys = vec![
            KeySlot {
                id: "one".into(),
                label: "Main".into(),
            },
            KeySlot {
                id: "two".into(),
                label: "Backup".into(),
            },
        ];
        c.active_profile_id = "main".into();
        c
    }

    fn key(account: &str) -> Result<Option<String>, String> {
        Ok(Some(
            if account.ends_with(":one") {
                "fake-first"
            } else {
                "fake-second"
            }
            .into(),
        ))
    }
    fn answer(text: &str) -> String {
        serde_json::json!({"choices":[{"message":{"content":text}}]}).to_string()
    }

    #[tokio::test]
    async fn failed_key_retries_without_duplicate_history_and_masks_error_bodies() {
        let (url, rx, thread) = server(vec![
            (
                401,
                "{\"error\":\"fake-first should never be exposed\"}".into(),
            ),
            (200, answer("success")),
        ]);
        let c = config(url);
        let chat = Chat::default();
        let reply = send(&chat, &c, "hello".into(), None, key).await.unwrap();
        assert!(reply.used_fallback);
        assert_eq!(reply.key_label.as_deref(), Some("Backup"));
        let first = rx.recv().unwrap();
        let second = rx.recv().unwrap();
        thread.join().unwrap();
        assert!(first.contains("Bearer fake-first"));
        assert!(second.contains("Bearer fake-second"));
        assert_eq!(chat.messages.lock().unwrap().len(), 2);
        assert_eq!(second.matches("hello").count(), 1);
    }

    #[tokio::test]
    async fn exhausted_keys_move_to_the_explicit_model_fallback() {
        let (url, rx, thread) = server(vec![
            (429, "{}".into()),
            (503, "{}".into()),
            (200, answer("backup answer")),
        ]);
        let mut c = config(url);
        let mut fallback = c.profiles[0].clone();
        fallback.id = "backup".into();
        fallback.name = "Backup profile".into();
        fallback.model = "model-two".into();
        fallback.keys.truncate(1);
        c.profiles.push(fallback);
        c.fallback_enabled = true;
        c.fallback_profile_ids.push("backup".into());
        let chat = Chat::default();
        let reply = send(&chat, &c, "hello".into(), None, key).await.unwrap();
        assert_eq!(reply.profile_id, "backup");
        assert!(reply.used_fallback);
        rx.recv().unwrap();
        rx.recv().unwrap();
        assert!(rx.recv().unwrap().contains("model-two"));
        thread.join().unwrap();
    }

    #[tokio::test]
    async fn provider_protocols_send_expected_endpoints_auth_and_conversation_roles() {
        for (provider, response, endpoint, auth) in [
            (
                Provider::Anthropic,
                serde_json::json!({"content":[{"type":"text","text":"answer"}]}),
                "/v1/messages",
                "x-api-key: fake-key",
            ),
            (
                Provider::Gemini,
                serde_json::json!({"candidates":[{"content":{"parts":[{"text":"answer"}]}}]}),
                "/v1/models/chat-model:generateContent",
                "x-goog-api-key: fake-key",
            ),
            (
                Provider::Openai,
                serde_json::json!({"output":[{"type":"message","content":[{"type":"output_text","text":"answer"}]}]}),
                "/v1/responses",
                "authorization: Bearer fake-key",
            ),
            (
                Provider::Ollama,
                serde_json::json!({"message":{"content":"answer"}}),
                "/v1/api/chat",
                "",
            ),
            (
                Provider::LocalOpenai,
                serde_json::json!({"choices":[{"message":{"content":"answer"}}]}),
                "/v1/chat/completions",
                "",
            ),
        ] {
            let (url, rx, thread) = server(vec![(200, response.to_string())]);
            let mut profile = config(url).profiles.remove(0);
            profile.provider = provider;
            profile.model = "chat-model".into();
            let messages = vec![
                Message {
                    role: "user",
                    parts: vec![Part::Text("first question".into())],
                },
                Message {
                    role: "assistant",
                    parts: vec![Part::Text("first answer".into())],
                },
                Message {
                    role: "user",
                    parts: vec![Part::Text("follow up".into())],
                },
            ];
            let key = if provider.is_local() {
                None
            } else {
                Some("fake-key")
            };
            assert_eq!(
                providers::complete(&profile, key, &messages).await.unwrap(),
                "answer"
            );
            let request = rx.recv().unwrap();
            thread.join().unwrap();
            assert!(request.starts_with(&format!("POST {endpoint} HTTP/1.1")));
            assert!(request.contains(auth));
            assert!(
                request.contains("first question")
                    && request.contains("first answer")
                    && request.contains("follow up")
            );
            if provider == Provider::Gemini {
                assert!(request.contains("\"role\":\"model\""));
            }
            if provider.is_local() {
                assert!(!request.contains("fake-key"));
            }
        }
    }

    #[tokio::test]
    async fn selecting_another_model_keeps_successful_history_and_local_needs_no_vault() {
        let (url, rx, thread) = server(vec![
            (200, answer("first answer")),
            (200, answer("second answer")),
        ]);
        let mut c = config(url);
        c.profiles[0].provider = Provider::LocalOpenai;
        c.profiles[0].keys.clear();
        let chat = Chat::default();
        let no_vault = |_: &str| -> Result<Option<String>, String> {
            panic!("Unauthenticated local inference must not read the credential store")
        };
        send(&chat, &c, "first question".into(), None, no_vault)
            .await
            .unwrap();
        c.profiles[0].model = "another-model".into();
        send(&chat, &c, "follow up".into(), None, no_vault)
            .await
            .unwrap();
        rx.recv().unwrap();
        let second = rx.recv().unwrap();
        thread.join().unwrap();
        assert!(second.contains("another-model"));
        assert_eq!(second.matches("first question").count(), 1);
        assert_eq!(second.matches("first answer").count(), 1);
        assert_eq!(chat.messages.lock().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn refusal_stops_fallback_and_failed_turn_does_not_enter_history() {
        let (url, _, thread) = server(vec![(
            200,
            serde_json::json!({"choices":[{"finish_reason":"content_filter"}]}).to_string(),
        )]);
        let chat = Chat::default();
        let error = send(&chat, &config(url), "hello".into(), None, key)
            .await
            .unwrap_err();
        assert!(error.contains("declined"));
        assert!(chat.messages.lock().unwrap().is_empty());
        thread.join().unwrap();
    }

    #[tokio::test]
    async fn reset_cancels_an_inflight_turn_without_restoring_old_history() {
        let chat = Arc::new(Chat::default());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let c = config(format!("http://{}/v1", listener.local_addr().unwrap()));
        let sending = chat.clone();
        let task = tokio::spawn(async move { send(&sending, &c, "hello".into(), None, key).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        chat.reset();
        assert!(task.await.unwrap().unwrap_err().contains("cancelled"));
        assert!(chat.messages.lock().unwrap().is_empty());
    }
}
