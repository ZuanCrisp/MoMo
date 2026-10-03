//! Provider-specific tool envelopes keep signatures/reasoning intact per route.
use super::{
    chat::Message,
    config::{Profile, Provider},
    providers::{self, Failure, Retry},
};
use crate::desktop::Action;
use serde_json::{json, Value};
use std::sync::Mutex;

#[derive(Default)]
pub struct Journal {
    entries: Mutex<Vec<(String, Action)>>,
    pub route: Mutex<Option<super::chat::ChatReply>>,
}
impl Journal {
    pub fn actions(&self) -> Vec<Action> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .map(|(_, a)| a.clone())
            .collect()
    }
    fn apply<F>(&self, name: &str, args: &Value, execute: &F) -> Action
    where
        F: Fn(&str, &Value) -> Action,
    {
        let fingerprint = crate::desktop::fingerprint(name, args);
        let mut entries = self.entries.lock().unwrap();
        if let Some((_, action)) = entries.iter().find(|(key, _)| key == &fingerprint) {
            return action.clone();
        }
        let action = if entries.len() >= 8 {
            Action {
                name: name.into(),
                detail: "Desktop action limit reached for this request.".into(),
                success: false,
                path: None,
            }
        } else {
            execute(name, args)
        };
        entries.push((fingerprint, action.clone()));
        action
    }
}

pub async fn complete<F>(
    profile: &Profile,
    key: Option<&str>,
    messages: &[Message],
    journal: &Journal,
    execute: F,
) -> Result<String, Failure>
where
    F: Fn(&str, &Value) -> Action,
{
    let mut body = providers::body(profile, messages)?;
    let definitions = crate::desktop::definitions();
    let tools: Vec<_> = definitions.into_iter().map(|definition| match profile.provider {
        Provider::Anthropic => json!({"name":definition["name"],"description":definition["description"],"input_schema":definition["parameters"]}),
        Provider::Openai => { let mut tool = definition; tool["type"] = json!("function"); tool["strict"] = json!(true); tool },
            Provider::Gemini => {
                let mut definition = definition;
                // Gemini's OpenAPI Schema supports a subset of JSON Schema.
                definition["parameters"].as_object_mut().unwrap().remove("additionalProperties");
                definition
            },
        _ => json!({"type":"function","function":definition}),
    }).collect();
    if profile.provider == Provider::Gemini {
        // Older Gemini models cannot combine Google search and client functions.
        body["tools"] = json!([{"functionDeclarations":tools}]);
    } else {
        let existing = body
            .as_object_mut()
            .unwrap()
            .entry("tools")
            .or_insert_with(|| json!([]));
        existing.as_array_mut().unwrap().extend(tools);
    }
    let instruction = "Desktop tools are enabled. Use them only for an action the user requests. Never invent successful actions. You can open supported apps and create text notes; you cannot click, type into other apps, run commands or inspect files. Report actual tool results, including failures.";
    match profile.provider {
        Provider::Anthropic => {
            body["system"] = json!(format!(
                "{}\n{instruction}",
                body["system"].as_str().unwrap_or("")
            ))
        }
        Provider::Gemini => body["systemInstruction"]["parts"]
            .as_array_mut()
            .unwrap()
            .push(json!({"text":instruction})),
        Provider::Openai => {
            body["instructions"] = json!(format!(
                "{}\n{instruction}",
                body["instructions"].as_str().unwrap_or("")
            ));
            body["include"] = json!(["reasoning.encrypted_content"]);
        }
        _ => {
            body["messages"][0]["content"] = json!(format!(
                "{}\n{instruction}",
                body["messages"][0]["content"].as_str().unwrap_or("")
            ))
        }
    }
    for _ in 0..6 {
        let response = providers::request(profile, key, &body).await?;
        let calls = calls(profile.provider, &response)?;
        if calls.is_empty() {
            return providers::parse_reply(profile.provider, &response);
        }
        if calls.len() > 8 {
            return Err(Failure::new(
                "The model requested too many desktop actions.",
                Retry::Stop,
            ));
        }
        let results: Vec<_> = calls
            .iter()
            .map(|call| json!(journal.apply(&call.name, &call.args, &execute)))
            .collect();
        append_results(profile.provider, &mut body, &response, &calls, results);
    }
    Err(Failure::new(
        "Desktop action loop reached its limit.",
        Retry::Stop,
    ))
}

struct Call {
    id: String,
    name: String,
    args: Value,
}
fn calls(provider: Provider, response: &Value) -> Result<Vec<Call>, Failure> {
    // Surface refusals even if an unexpected response also contains a tool call.
    if let Err(error) = providers::parse_reply(provider, response) {
        if error.retry == Retry::Stop {
            return Err(error);
        }
    }
    let raw: Vec<&Value> = match provider {
        Provider::Anthropic => response["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|part| part["type"] == "tool_use")
            .collect(),
        Provider::Openai => response["output"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["type"] == "function_call")
            .collect(),
        Provider::Gemini => response["candidates"][0]["content"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|part| part["functionCall"].is_object())
            .map(|part| &part["functionCall"])
            .collect(),
        Provider::Ollama => response["message"]["tool_calls"]
            .as_array()
            .into_iter()
            .flatten()
            .collect(),
        _ => response["choices"][0]["message"]["tool_calls"]
            .as_array()
            .into_iter()
            .flatten()
            .collect(),
    };
    raw.into_iter()
        .enumerate()
        .map(|(index, raw)| {
            let (name, args, id) = match provider {
                Provider::Anthropic => (&raw["name"], &raw["input"], &raw["id"]),
                Provider::Gemini => (&raw["name"], &raw["args"], &raw["id"]),
                Provider::Openai => (&raw["name"], &raw["arguments"], &raw["call_id"]),
                _ => (
                    &raw["function"]["name"],
                    &raw["function"]["arguments"],
                    &raw["id"],
                ),
            };
            let args = if let Some(text) = args.as_str() {
                serde_json::from_str(text).map_err(|_| {
                    Failure::new("The model returned invalid action arguments.", Retry::Stop)
                })?
            } else {
                args.clone()
            };
            if !args.is_object() || name.as_str().is_none() {
                return Err(Failure::new(
                    "The model returned an invalid desktop action.",
                    Retry::Stop,
                ));
            }
            Ok(Call {
                id: id
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("action-{index}")),
                name: name.as_str().unwrap().to_owned(),
                args,
            })
        })
        .collect()
}

fn append_results(
    provider: Provider,
    body: &mut Value,
    response: &Value,
    calls: &[Call],
    results: Vec<Value>,
) {
    match provider {
        Provider::Anthropic => {
            let messages = body["messages"].as_array_mut().unwrap();
            messages.push(json!({"role":"assistant","content":response["content"]}));
            messages.push(json!({"role":"user","content":calls.iter().zip(results).map(|(call,result)| json!({"type":"tool_result","tool_use_id":call.id,"is_error":result["success"] != true,"content":result.to_string()})).collect::<Vec<_>>()}));
        }
        Provider::Gemini => {
            let contents = body["contents"].as_array_mut().unwrap();
            // Preserve all returned parts, including thoughtSignature.
            contents.push(response["candidates"][0]["content"].clone());
            contents.push(json!({"role":"user","parts":calls.iter().zip(results).map(|(call,result)| { let mut part = json!({"functionResponse":{"name":call.name,"response":result}}); if !call.id.starts_with("action-") { part["functionResponse"]["id"] = json!(call.id); } part }).collect::<Vec<_>>()}));
        }
        Provider::Openai => {
            let input = body["input"].as_array_mut().unwrap();
            input.extend(response["output"].as_array().into_iter().flatten().cloned());
            input.extend(calls.iter().zip(results).map(|(call,result)| json!({"type":"function_call_output","call_id":call.id,"output":result.to_string()})));
        }
        _ => {
            let messages = body["messages"].as_array_mut().unwrap();
            messages.push(if provider == Provider::Ollama {
                response["message"].clone()
            } else {
                response["choices"][0]["message"].clone()
            });
            messages.extend(calls.iter().zip(results).map(|(call, result)| {
                if provider == Provider::Ollama {
                    json!({"role":"tool","tool_name":call.name,"content":result.to_string()})
                } else {
                    json!({"role":"tool","tool_call_id":call.id,"content":result.to_string()})
                }
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tool_results_keep_provider_call_ids_and_gemini_signatures() {
        for (provider, response, field, expected) in [
            (
                Provider::Anthropic,
                json!({"content":[{"type":"tool_use","id":"call-1","name":"open_app","input":{"app":"notepad"}}]}),
                "messages",
                "tool_use_id",
            ),
            (
                Provider::Gemini,
                json!({"candidates":[{"content":{"role":"model","parts":[{"functionCall":{"id":"call-1","name":"open_app","args":{"app":"notepad"}},"thoughtSignature":"signature-to-preserve"}]}}]}),
                "contents",
                "signature-to-preserve",
            ),
            (
                Provider::Openai,
                json!({"output":[{"type":"function_call","call_id":"call-1","name":"open_app","arguments":"{\"app\":\"notepad\"}"}]}),
                "input",
                "function_call_output",
            ),
            (
                Provider::LocalOpenai,
                json!({"choices":[{"message":{"role":"assistant","tool_calls":[{"id":"call-1","type":"function","function":{"name":"open_app","arguments":"{\"app\":\"notepad\"}"}}]}}]}),
                "messages",
                "tool_call_id",
            ),
            (
                Provider::Ollama,
                json!({"message":{"role":"assistant","tool_calls":[{"function":{"name":"open_app","arguments":{"app":"notepad"}}}]}}),
                "messages",
                "tool_name",
            ),
        ] {
            let calls = calls(provider, &response).unwrap();
            assert_eq!(calls.len(), 1);
            let mut body = json!({field:[]});
            append_results(
                provider,
                &mut body,
                &response,
                &calls,
                vec![json!({"success":true,"detail":"Opened Notepad"})],
            );
            assert!(body.to_string().contains(expected));
            assert!(body.to_string().contains("Opened Notepad"));
        }
    }
    #[test]
    fn repeated_calls_are_executed_once_per_turn() {
        let count = std::cell::Cell::new(0);
        let execute = |name: &str, _: &Value| {
            count.set(count.get() + 1);
            Action {
                name: name.into(),
                detail: "Opened".into(),
                success: true,
                path: None,
            }
        };
        let journal = Journal::default();
        journal.apply("open_app", &json!({"app":"notepad"}), &execute);
        journal.apply("open_app", &json!({"app":"notepad"}), &execute);
        assert_eq!(count.get(), 1);
        assert_eq!(journal.actions().len(), 1);
    }
}
