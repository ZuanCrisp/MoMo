use std::time::Duration;

use reqwest::{Client, RequestBuilder};
use serde::Serialize;
use serde_json::{json, Value};

use super::chat::{Message, Part};
use super::config::{Profile, Provider};

const SYSTEM: &str = "You are MoMo, a helpful personal AI assistant on the user's desktop. Respond in the user's language. Use plain text with line breaks. Use web search when a search tool is available and needed; otherwise do not claim to have searched the web.";
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    Key,
    Profile,
    Stop,
}

#[derive(Debug)]
pub struct Failure {
    pub message: String,
    pub retry: Retry,
}

impl Failure {
    pub fn new(message: impl Into<String>, retry: Retry) -> Self {
        Self {
            message: message.into(),
            retry,
        }
    }
}

pub fn client(timeout: u64, local: bool) -> Result<Client, Failure> {
    let builder = Client::builder()
        .timeout(Duration::from_secs(timeout))
        .connect_timeout(Duration::from_secs(10))
        // Custom providers must not forward API keys through an HTTP redirect.
        .redirect(reqwest::redirect::Policy::none());
    // Local inference stays on loopback even on machines with a system proxy.
    let builder = if local { builder.no_proxy() } else { builder };
    #[cfg(test)]
    let builder = builder.no_proxy();
    builder
        .build()
        .map_err(|_| Failure::new("Could not create the network connection.", Retry::Stop))
}

fn authenticate(request: RequestBuilder, provider: Provider, key: Option<&str>) -> RequestBuilder {
    let request = if let Some(key) = key {
        match provider {
            Provider::Anthropic => request.header("x-api-key", key),
            Provider::Gemini => request.header("x-goog-api-key", key),
            _ => request.bearer_auth(key),
        }
    } else {
        request
    };
    if provider == Provider::Anthropic {
        request.header("anthropic-version", "2023-06-01")
    } else {
        request
    }
}

pub async fn json_response(request: RequestBuilder) -> Result<Value, Failure> {
    let mut response = request.send().await.map_err(|error| {
        let message = if error.is_timeout() {
            "Request timed out. Check the connection or increase this profile's timeout."
        } else {
            "Cannot reach the AI server. Check its URL and whether it is running."
        };
        Failure::new(message, Retry::Key)
    })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        // Never expose the response body: some servers echo credentials or input.
        let (message, retry) = match status {
            401 | 403 => ("API key rejected or access denied", Retry::Key),
            402 => ("API credit or billing unavailable", Retry::Key),
            404 => ("Model or API endpoint not found", Retry::Profile),
            408 => ("AI server timed out", Retry::Key),
            429 => ("API rate limit or quota reached", Retry::Key),
            500..=599 => ("AI service temporarily unavailable", Retry::Key),
            300..=399 => (
                "Server redirected the request. Check the final API base URL",
                Retry::Stop,
            ),
            _ => (
                "Request rejected. Check the model, output limit and supported features",
                Retry::Stop,
            ),
        };
        return Err(Failure::new(format!("{message} (HTTP {status})."), retry));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| Failure::new("Could not read the AI response.", Retry::Key))?
    {
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(Failure::new(
                "AI response exceeded the size limit.",
                Retry::Stop,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        Failure::new(
            "The AI server returned invalid JSON. Check the API base URL.",
            Retry::Profile,
        )
    })
}

fn text(message: &Message) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| {
            if let Part::Text(text) = part {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn body(profile: &Profile, messages: &[Message]) -> Result<Value, Failure> {
    if !matches!(
        profile.provider,
        Provider::Anthropic | Provider::Gemini | Provider::Openai
    ) && messages
        .iter()
        .flat_map(|m| &m.parts)
        .any(|part| matches!(part, Part::File { mime, .. } if !mime.starts_with("image/")))
    {
        return Err(Failure::new("PDF attachments need a Claude, Gemini or OpenAI profile. This profile supports text and images.", Retry::Profile));
    }
    let mut body = match profile.provider {
        Provider::Anthropic => {
            let messages: Vec<_> = messages.iter().map(|m| {
                let content: Vec<_> = m.parts.iter().map(|part| match part {
                    Part::Text(text) => json!({"type":"text","text":text}),
                    Part::File { mime, data, .. } => json!({"type": if mime.starts_with("image/") {"image"} else {"document"}, "source":{"type":"base64","media_type":mime,"data":data}}),
                }).collect();
                json!({"role":m.role,"content":content})
            }).collect();
            json!({"model":profile.model,"max_tokens":profile.max_output_tokens,"system":SYSTEM,"messages":messages})
        }
        Provider::Gemini => {
            let contents: Vec<_> = messages
                .iter()
                .map(|m| {
                    let parts: Vec<_> = m
                        .parts
                        .iter()
                        .map(|part| match part {
                            Part::Text(text) => json!({"text":text}),
                            Part::File { mime, data, .. } => {
                                json!({"inlineData":{"mimeType":mime,"data":data}})
                            }
                        })
                        .collect();
                    json!({"role":if m.role == "assistant" {"model"} else {"user"}, "parts":parts})
                })
                .collect();
            json!({"contents":contents,"systemInstruction":{"parts":[{"text":SYSTEM}]},"generationConfig":{"maxOutputTokens":profile.max_output_tokens}})
        }
        Provider::Openai => {
            let input: Vec<_> = messages.iter().map(|m| {
                if m.role == "assistant" { return json!({"role":"assistant","content":text(m)}); }
                let content: Vec<_> = m.parts.iter().map(|part| match part {
                    Part::Text(text) => json!({"type":"input_text","text":text}),
                    Part::File { mime, data, .. } if mime.starts_with("image/") => json!({"type":"input_image","image_url":format!("data:{mime};base64,{data}")}),
                    Part::File { mime, data, name } => json!({"type":"input_file","filename":name,"file_data":format!("data:{mime};base64,{data}")}),
                }).collect();
                json!({"role":"user","content":content})
            }).collect();
            json!({"model":profile.model,"instructions":SYSTEM,"input":input,"max_output_tokens":profile.max_output_tokens,"store":false})
        }
        Provider::OpenaiCompatible | Provider::LocalOpenai => {
            let mut converted = vec![json!({"role":"system","content":SYSTEM})];
            for m in messages {
                let content: Vec<_> = m.parts.iter().map(|part| match part {
                    Part::Text(text) => json!({"type":"text","text":text}),
                    Part::File { mime, data, .. } => json!({"type":"image_url","image_url":{"url":format!("data:{mime};base64,{data}")}}),
                }).collect();
                if m.parts.iter().all(|part| matches!(part, Part::Text(_))) {
                    converted.push(json!({"role":m.role,"content":text(m)}));
                } else {
                    converted.push(json!({"role":m.role,"content":content}));
                }
            }
            json!({"model":profile.model,"messages":converted,"max_tokens":profile.max_output_tokens,"stream":false})
        }
        Provider::Ollama => {
            let mut converted = vec![json!({"role":"system","content":SYSTEM})];
            for m in messages {
                let mut item = json!({"role":m.role,"content":text(m)});
                let images: Vec<_> = m
                    .parts
                    .iter()
                    .filter_map(|part| match part {
                        Part::File { data, .. } => Some(data),
                        _ => None,
                    })
                    .collect();
                if !images.is_empty() {
                    item["images"] = json!(images);
                }
                converted.push(item);
            }
            json!({"model":profile.model,"messages":converted,"options":{"num_predict":profile.max_output_tokens},"stream":false})
        }
    };
    if profile.web_search {
        body["tools"] = match profile.provider {
            Provider::Anthropic => {
                json!([{"type":"web_search_20250305","name":"web_search","max_uses":5}])
            }
            Provider::Openai => json!([{"type":"web_search"}]),
            Provider::Gemini => json!([{"googleSearch":{}}]),
            _ => {
                return Err(Failure::new(
                    "This provider does not support built-in web search.",
                    Retry::Stop,
                ))
            }
        };
    }
    Ok(body)
}

pub fn parse_reply(provider: Provider, response: &Value) -> Result<String, Failure> {
    let refused = match provider {
        Provider::Anthropic => response["stop_reason"] == "refusal",
        Provider::Gemini => {
            response["promptFeedback"]["blockReason"].is_string()
                || response["candidates"].as_array().is_some_and(|items| {
                    items.iter().any(|item| {
                        matches!(
                            item["finishReason"].as_str(),
                            Some(
                                "SAFETY"
                                    | "RECITATION"
                                    | "BLOCKLIST"
                                    | "PROHIBITED_CONTENT"
                                    | "SPII"
                            )
                        )
                    })
                })
        }
        Provider::Openai => response["output"].as_array().is_some_and(|items| {
            items.iter().any(|item| {
                item["content"]
                    .as_array()
                    .is_some_and(|parts| parts.iter().any(|part| part["type"] == "refusal"))
            })
        }),
        _ => {
            response["choices"][0]["message"]["refusal"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
                || response["choices"][0]["finish_reason"] == "content_filter"
        }
    };
    if refused {
        return Err(Failure::new(
            "The provider declined this request. Try changing your question. No fallback was sent.",
            Retry::Stop,
        ));
    }
    let parts: Vec<&str> = match provider {
        Provider::Anthropic => response["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect(),
        Provider::Gemini => response["candidates"][0]["content"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b["thought"] != true)
            .filter_map(|b| b["text"].as_str())
            .collect(),
        Provider::Openai => response["output"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["type"] == "message")
            .flat_map(|item| item["content"].as_array().into_iter().flatten())
            .filter(|part| part["type"] == "output_text")
            .filter_map(|part| part["text"].as_str())
            .collect(),
        Provider::Ollama => response["message"]["content"]
            .as_str()
            .into_iter()
            .collect(),
        _ => response["choices"][0]["message"]["content"]
            .as_str()
            .into_iter()
            .collect(),
    };
    let text = parts.join("\n").trim().to_string();
    if text.is_empty() {
        if response["status"] == "incomplete"
            || response["candidates"][0]["finishReason"] == "MAX_TOKENS"
        {
            return Err(Failure::new("The model reached its output limit without a text reply. Increase the output limit in Advanced settings.", Retry::Stop));
        }
        return Err(Failure::new(
            "The AI server returned no text. Check that this model supports chat.",
            Retry::Profile,
        ));
    }
    Ok(text)
}

pub async fn complete(
    profile: &Profile,
    key: Option<&str>,
    messages: &[Message],
) -> Result<String, Failure> {
    let body = body(profile, messages)?;
    let response = request(profile, key, &body).await?;
    parse_reply(profile.provider, &response)
}

pub async fn request(profile: &Profile, key: Option<&str>, body: &Value) -> Result<Value, Failure> {
    let suffix = match profile.provider {
        Provider::Anthropic => "messages".into(),
        Provider::Openai => "responses".into(),
        Provider::Gemini => format!("models/{}:generateContent", profile.model),
        Provider::Ollama => "api/chat".into(),
        _ => "chat/completions".into(),
    };
    let request = client(profile.timeout_seconds, profile.provider.is_local())?
        .post(format!("{}/{suffix}", profile.base_url))
        .json(&body);
    json_response(authenticate(request, profile.provider, key)).await
}

#[derive(Clone, Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub label: String,
}

pub fn parse_models(provider: Provider, value: &Value) -> Vec<Model> {
    let items = if matches!(provider, Provider::Gemini | Provider::Ollama) {
        &value["models"]
    } else {
        &value["data"]
    };
    let mut models: Vec<_> = items
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = if matches!(provider, Provider::Gemini | Provider::Ollama) {
                item["name"].as_str()?
            } else {
                item["id"].as_str()?
            };
            let id = id.strip_prefix("models/").unwrap_or(id);
            if provider == Provider::Gemini
                && !item["supportedGenerationMethods"]
                    .as_array()
                    .is_some_and(|methods| methods.iter().any(|m| m == "generateContent"))
            {
                return None;
            }
            if provider == Provider::Ollama
                && (id.to_ascii_lowercase().contains("cloud")
                    || item["remote_host"].is_string()
                    || item["remote_model"].is_string())
            {
                return None;
            }
            if provider == Provider::Openai
                && [
                    "embedding",
                    "whisper",
                    "tts-",
                    "dall-e",
                    "moderation",
                    "gpt-image",
                    "realtime",
                    "transcribe",
                    "sora",
                ]
                .iter()
                .any(|family| id.contains(family))
            {
                return None;
            }
            let label = item["display_name"]
                .as_str()
                .or(item["displayName"].as_str())
                .unwrap_or(id);
            Some(Model {
                id: id.into(),
                label: label.into(),
            })
        })
        .collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|a, b| a.id == b.id);
    models
}

pub async fn models(profile: &Profile, key: Option<&str>) -> Result<Vec<Model>, Failure> {
    let suffix = match profile.provider {
        Provider::Anthropic => "models?limit=100",
        Provider::Gemini => "models?pageSize=1000",
        Provider::Ollama => "api/tags",
        _ => "models",
    };
    let request =
        client(10, profile.provider.is_local())?.get(format!("{}/{suffix}", profile.base_url));
    let value = json_response(authenticate(request, profile.provider, key)).await?;
    Ok(parse_models(profile.provider, &value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::config::Config;

    #[test]
    fn adapters_preserve_history_images_and_pdf_capabilities() {
        let mut profile = Config::default().profiles.remove(0);
        let messages = vec![Message {
            role: "user",
            parts: vec![
                Part::Text("question".into()),
                Part::File {
                    mime: "application/pdf".into(),
                    data: "Zm9v".into(),
                    name: "report.pdf".into(),
                },
            ],
        }];
        for provider in [Provider::Anthropic, Provider::Gemini, Provider::Openai] {
            profile.provider = provider;
            profile.web_search = false;
            let serialized = body(&profile, &messages).unwrap().to_string();
            assert!(serialized.contains("Zm9v"));
            assert!(serialized.contains("question"));
        }
        for provider in [
            Provider::OpenaiCompatible,
            Provider::LocalOpenai,
            Provider::Ollama,
        ] {
            profile.provider = provider;
            assert_eq!(body(&profile, &messages).unwrap_err().retry, Retry::Profile);
        }
        profile.provider = Provider::Openai;
        let request = body(&profile, &messages).unwrap();
        assert_eq!(request["store"], false);
    }

    #[test]
    fn parsers_find_text_after_reasoning_and_stop_at_refusals() {
        let response = json!({"output":[{"type":"reasoning"},{"type":"message","content":[{"type":"output_text","text":"hello"}]}]});
        assert_eq!(parse_reply(Provider::Openai, &response).unwrap(), "hello");
        let gemini = json!({"candidates":[{"content":{"parts":[{"text":"private reasoning","thought":true},{"text":"answer"}]}}]});
        assert_eq!(parse_reply(Provider::Gemini, &gemini).unwrap(), "answer");
        for (provider, response) in [
            (Provider::Anthropic, json!({"stop_reason":"refusal"})),
            (
                Provider::Gemini,
                json!({"promptFeedback":{"blockReason":"SAFETY"}}),
            ),
            (
                Provider::Openai,
                json!({"output":[{"content":[{"type":"refusal"}]}]}),
            ),
            (
                Provider::LocalOpenai,
                json!({"choices":[{"finish_reason":"content_filter"}]}),
            ),
        ] {
            assert_eq!(
                parse_reply(provider, &response).unwrap_err().retry,
                Retry::Stop
            );
        }
    }

    #[test]
    fn model_discovery_filters_non_chat_and_cloud_models() {
        let gemini = json!({"models":[{"name":"models/chat-model","supportedGenerationMethods":["generateContent"]},{"name":"models/embed","supportedGenerationMethods":["embedContent"]}]});
        assert_eq!(parse_models(Provider::Gemini, &gemini)[0].id, "chat-model");
        let ollama = json!({"models":[{"name":"llama3.2"},{"name":"model:cloud"},{"name":"remote","remote_host":"example.com"}]});
        assert_eq!(parse_models(Provider::Ollama, &ollama).len(), 1);
    }
}
