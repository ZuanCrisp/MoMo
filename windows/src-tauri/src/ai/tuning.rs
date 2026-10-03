//! Model-aware Ollama controls. Unknown reasoning capabilities use model defaults.
use super::{
    config::{Profile, Provider},
    providers::{self, Failure, Retry},
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDetails {
    pub size_bytes: u64,
    pub max_context_tokens: u64,
    pub parameters: String,
    pub quantization: String,
    pub capabilities: Vec<String>,
    pub thinking_values: Vec<Value>,
    pub legacy_thinking_controls: bool,
}
type Cache = HashMap<String, (Instant, ModelDetails)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

pub fn parse(show: &Value, size: u64) -> ModelDetails {
    let capabilities: Vec<String> = show["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let metadata = show["thinking"]["values"].as_array();
    let family = show["details"]["family"].as_str().unwrap_or("");
    // Older installed Ollama releases expose capabilities but omit values.
    // Use only known family controls; other families retain their own defaults.
    let legacy = metadata.is_none() && capabilities.iter().any(|v| v == "thinking");
    let values = metadata.cloned().unwrap_or_else(|| {
        if !legacy {
            return vec![];
        }
        match family {
            "gptoss" => vec![json!("low"), json!("medium"), json!("high")],
            "qwen3" | "qwen35" => vec![json!(false), json!(true)],
            _ => vec![],
        }
    });
    ModelDetails {
        size_bytes: size,
        max_context_tokens: show["model_info"]
            .as_object()
            .into_iter()
            .flat_map(|v| v.iter())
            .filter(|(k, _)| k.ends_with(".context_length"))
            .filter_map(|(_, v)| v.as_u64())
            .max()
            .unwrap_or(0),
        parameters: show["details"]["parameter_size"]
            .as_str()
            .unwrap_or("Unknown")
            .into(),
        quantization: show["details"]["quantization_level"]
            .as_str()
            .unwrap_or("Unknown")
            .into(),
        capabilities,
        thinking_values: values.clone(),
        legacy_thinking_controls: legacy && !values.is_empty(),
    }
}
pub async fn details(profile: &Profile, key: Option<&str>) -> Result<ModelDetails, Failure> {
    if profile.provider == Provider::LocalOpenai || profile.provider == Provider::LocalResponses {
        return lm_details(profile, key).await;
    }
    if profile.provider != Provider::Ollama {
        return Err(Failure::new(
            "Model inspection is available for Ollama profiles.",
            Retry::Stop,
        ));
    }
    let cache_key = format!("{}:{}:{}", profile.id, profile.base_url, profile.model);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((when, model)) = cache.lock().unwrap().get(&cache_key) {
        if when.elapsed() < Duration::from_secs(60) {
            return Ok(model.clone());
        }
    }
    let client = providers::client(12, true)?;
    let mut request = client
        .post(format!("{}/api/show", profile.base_url))
        .json(&json!({"model":profile.model}));
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let show = providers::json_response(request).await?;
    let mut request = client.get(format!("{}/api/tags", profile.base_url));
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let tags = providers::json_response(request)
        .await
        .unwrap_or(Value::Null);
    let size = tags["models"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|v| v["name"] == profile.model || v["model"] == profile.model)
        .and_then(|v| v["size"].as_u64())
        .unwrap_or(0);
    let model = parse(&show, size);
    let mut cache = cache.lock().unwrap();
    if cache.len() >= 64 {
        cache.clear();
    }
    cache.insert(cache_key, (Instant::now(), model.clone()));
    Ok(model)
}
pub async fn lm_details(profile: &Profile, key: Option<&str>) -> Result<ModelDetails, Failure> {
    let cache_key = format!("{}:{}:{}", profile.id, profile.base_url, profile.model);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((when, model)) = cache.lock().unwrap().get(&cache_key) {
        if when.elapsed() < Duration::from_secs(60) {
            return Ok(model.clone());
        }
    }
    let base = profile.base_url.trim_end_matches("/v1");
    let mut request = providers::client(5, true)?.get(format!("{base}/api/v1/models"));
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    let response = providers::json_response(request).await?;
    let model = response["models"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|m| {
            m["key"] == profile.model
                || m["loaded_instances"]
                    .as_array()
                    .is_some_and(|instances| instances.iter().any(|i| i["id"] == profile.model))
        })
        .ok_or_else(|| {
            Failure::new(
                "Model metadata was not found. Use the model key from LM Studio.",
                Retry::Profile,
            )
        })?;
    let values: Vec<Value> = model["capabilities"]["reasoning"]["allowed_options"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| ["low", "medium", "high"].iter().any(|s| **v == json!(s)))
        .cloned()
        .collect();
    let mut capabilities = vec!["completion".into()];
    if model["capabilities"]["trained_for_tool_use"] == true {
        capabilities.push("tools".into());
    }
    if model["capabilities"]["vision"] == true {
        capabilities.push("vision".into());
    }
    if !values.is_empty() {
        capabilities.push("thinking".into());
    }
    let details = ModelDetails {
        size_bytes: model["size_bytes"].as_u64().unwrap_or(0),
        max_context_tokens: model["max_context_length"].as_u64().unwrap_or(0),
        parameters: model["params_string"].as_str().unwrap_or("Unknown").into(),
        quantization: model["quantization"]["name"]
            .as_str()
            .unwrap_or("Unknown")
            .into(),
        capabilities,
        thinking_values: values,
        legacy_thinking_controls: false,
    };
    let mut cache = cache.lock().unwrap();
    if cache.len() >= 64 {
        cache.clear();
    }
    cache.insert(cache_key, (Instant::now(), details.clone()));
    Ok(details)
}
pub async fn wire_profile(profile: &Profile, key: Option<&str>) -> Result<Profile, Failure> {
    let mut wire = profile.clone();
    if profile.provider == Provider::LocalOpenai && profile.reasoning != "default" {
        match lm_details(profile, key).await {
            Ok(details) if !details.thinking_values.is_empty() => wire.provider = Provider::LocalResponses,
            _ if profile.reasoning == "adaptive" => {},
            _ => return Err(Failure::new("This local server does not expose that reasoning control. Use Adaptive or Model default, or configure reasoning in the server.", Retry::Stop)),
        }
    }
    Ok(wire)
}
fn complex(query: &str) -> bool {
    // Opening an app or copying a note benefits from the fastest supported
    // mode, even when the requested note happens to mention "reasoning".
    if super::desktop_tools::requested(query) {
        return false;
    }
    let query = query.to_lowercase();
    query.chars().count() > 240
        || query.contains("```")
        || [
            "analisis",
            "analyze",
            "analyse",
            "reason",
            "buktikan",
            "prove",
            "debug",
            "algorit",
            "algorithm",
            "bandingkan",
            "compare",
            "hitung",
            "calculate",
            "matemat",
            "step by step",
            "langkah demi langkah",
            "rancang",
            "explain why",
        ]
        .iter()
        .any(|v| query.contains(v))
}
pub fn reasoning(mode: &str, values: &[Value], query: &str) -> Option<Value> {
    let choice = match mode {
        "default" => return None,
        "adaptive" if values.contains(&json!(false)) && values.contains(&json!(true)) => {
            json!(complex(query))
        }
        "adaptive" if values.contains(&json!("low")) => {
            if complex(query) && values.contains(&json!("medium")) {
                json!("medium")
            } else {
                json!("low")
            }
        }
        "adaptive" => return None,
        "off" => json!(false),
        "on" => json!(true),
        value => json!(value),
    };
    values.contains(&choice).then_some(choice)
}
pub async fn prepare(
    profile: &Profile,
    key: Option<&str>,
    body: &mut Value,
) -> Result<(), Failure> {
    if profile.provider == Provider::LocalResponses {
        let info = lm_details(profile, key).await?;
        let query = body["input"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find(|m| m["role"] == "user")
            .map(|m| m["content"].to_string())
            .unwrap_or_default();
        if let Some(effort) = reasoning(&profile.reasoning, &info.thinking_values, &query) {
            body["reasoning"] = json!({"effort":effort});
        }
        return Ok(());
    }
    if profile.provider != Provider::Ollama {
        return Ok(());
    }
    body["options"]["num_ctx"] = json!(profile.local_context_tokens);
    body["keep_alive"] = json!(profile.keep_alive_minutes * 60);
    if profile.reasoning == "default" {
        return Ok(());
    }
    let info = details(profile, key).await?;
    if info.max_context_tokens > 0 {
        body["options"]["num_ctx"] =
            json!((profile.local_context_tokens as u64).min(info.max_context_tokens));
    }
    let query = body["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|v| v["role"] == "user")
        .and_then(|v| v["content"].as_str())
        .unwrap_or("");
    if let Some(think) = reasoning(&profile.reasoning, &info.thinking_values, query) {
        body["think"] = think;
    } else if profile.reasoning != "adaptive" {
        return Err(Failure::new("This model does not expose that reasoning option. Inspect the model and choose a supported value or Model default.", Retry::Stop));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adaptive_uses_supported_controls_without_inventing_unknown_ones() {
        assert_eq!(
            reasoning(
                "adaptive",
                &[json!(false), json!(true)],
                "Buat catatan berisi analisis dan adaptive reasoning"
            ),
            Some(json!(false))
        );
        assert_eq!(
            reasoning("adaptive", &[json!(false), json!(true)], "hello"),
            Some(json!(false))
        );
        assert_eq!(
            reasoning(
                "adaptive",
                &[json!(false), json!(true)],
                "analisis algoritma ini"
            ),
            Some(json!(true))
        );
        assert_eq!(
            reasoning("off", &[json!("low"), json!("medium")], "hello"),
            None
        );
        assert_eq!(
            reasoning("adaptive", &[json!("low"), json!("medium")], "hello"),
            Some(json!("low"))
        );
        assert_eq!(
            reasoning(
                "adaptive",
                &[json!("low"), json!("medium")],
                "calculate this"
            ),
            Some(json!("medium"))
        );
        assert_eq!(reasoning("adaptive", &[], "calculate this"), None);
        let model = parse(
            &json!({"capabilities":["thinking"],"details":{"family":"unknown"}}),
            42,
        );
        assert!(model.thinking_values.is_empty());
        assert_eq!(model.size_bytes, 42);
        assert_eq!(
            parse(
                &json!({"thinking":{"values":[false]},"details":{"family":"gptoss"}}),
                0
            )
            .thinking_values,
            vec![json!(false)]
        );
    }
}
