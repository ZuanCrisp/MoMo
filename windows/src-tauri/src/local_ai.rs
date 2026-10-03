//! MoMo owns a separate loopback Ollama server, leaving an existing server alone.
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;

pub const BASE_URL: &str = "http://127.0.0.1:11435";

#[derive(Default)]
pub struct LocalAI {
    child: Mutex<Option<Child>>,
    directory: Mutex<String>,
    starting: tokio::sync::Mutex<()>,
}

impl Drop for LocalAI {
    fn drop(&mut self) {
        if let Some(child) = self.child.get_mut().unwrap().as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl LocalAI {
    pub fn stop(&self) {
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub directory: String,
    pub models: Vec<String>,
    pub runtime_installed: bool,
    pub running: bool,
    pub base_url: &'static str,
}

fn runtime() -> Option<PathBuf> {
    let filename = if cfg!(windows) {
        "ollama.exe"
    } else {
        "ollama"
    };
    let mut candidates = Vec::new();
    #[cfg(windows)]
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(base).join("Programs/Ollama/ollama.exe"));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|p| p.join(filename)));
    }
    candidates.into_iter().find(|p| p.is_file())
}

pub fn discover(saved: &str) -> String {
    if !saved.trim().is_empty() {
        return saved.to_string();
    }
    let mut roots = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        roots.extend(exe.ancestors().skip(1).take(5).map(Path::to_path_buf));
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    // A locally built executable can find its source's library on first launch.
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    roots
        .into_iter()
        .map(|p| p.join("Local AI Models"))
        .find(|p| p.join("manifests").is_dir() && p.join("blobs").is_dir())
        .and_then(|p| p.canonicalize().ok())
        .map(|p| display_path(&p))
        .unwrap_or_default()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_string()
}

pub fn model_names(directory: &Path) -> Result<Vec<String>, String> {
    let library = directory.join("manifests/registry.ollama.ai/library");
    if !directory.join("blobs").is_dir() || !library.is_dir() {
        return Err("Choose your Ollama models folder containing blobs and manifests. GGUF files need importing into Ollama or loading in LM Studio first.".into());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(library)
        .map_err(|_| "Cannot read this model folder.")?
        .flatten()
        .take(256)
    {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains("embed") || name.contains("cloud") {
            continue;
        }
        for tag in std::fs::read_dir(entry.path())
            .map_err(|_| "Cannot read a model manifest.")?
            .flatten()
            .take(128)
        {
            if !tag.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }
            let tag_name = tag.file_name().to_string_lossy().to_string();
            if tag.metadata().is_ok_and(|meta| meta.len() > 1024 * 1024) {
                continue;
            }
            let bytes = std::fs::read(tag.path()).map_err(|_| "Cannot read a model manifest.")?;
            if bytes.len() > 1024 * 1024 {
                continue;
            }
            let manifest: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| "A model manifest is invalid.")?;
            let mut complete = true;
            for item in std::iter::once(&manifest["config"])
                .chain(manifest["layers"].as_array().into_iter().flatten())
            {
                let digest = item["digest"].as_str().unwrap_or("");
                if !digest.starts_with("sha256:")
                    || digest.len() != 71
                    || !digest[7..].chars().all(|c| c.is_ascii_hexdigit())
                    || !directory
                        .join("blobs")
                        .join(digest.replace(':', "-"))
                        .is_file()
                {
                    complete = false;
                    break;
                }
            }
            if complete {
                names.push(format!("{name}:{tag_name}"));
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

pub async fn status(state: &LocalAI, directory: &str) -> Result<Status, String> {
    let directory = discover(directory);
    let models = if directory.is_empty() {
        vec![]
    } else {
        model_names(Path::new(&directory))?
    };
    let owned = state.directory.lock().unwrap().as_str() == directory
        && state
            .child
            .lock()
            .unwrap()
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_none());
    let running = owned && healthy().await;
    Ok(Status {
        directory,
        models,
        runtime_installed: runtime().is_some(),
        running,
        base_url: BASE_URL,
    })
}

async fn healthy() -> bool {
    let Ok(client) = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
    else {
        return false;
    };
    client
        .get(format!("{BASE_URL}/api/tags"))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

pub async fn start(state: &LocalAI, directory: &str) -> Result<Status, String> {
    let _starting = state
        .starting
        .try_lock()
        .map_err(|_| "Local AI is already starting.")?;
    let directory = discover(directory);
    let path = Path::new(&directory)
        .canonicalize()
        .map_err(|_| "Choose an existing model folder.")?;
    let names = model_names(&path)?;
    if names.is_empty() {
        return Err("No complete chat models found in this folder. Check that both manifests and blobs were copied.".into());
    }
    let exe = runtime().ok_or("Ollama is not installed. Install Ollama, then return here; your existing model files will be reused.")?;
    let directory = display_path(&path);
    let same_directory = state.directory.lock().unwrap().as_str() == directory;
    if same_directory && healthy().await {
        return status(state, &directory).await;
    }
    {
        let mut child = state.child.lock().unwrap();
        if let Some(mut previous) = child.take() {
            let _ = previous.kill();
            let _ = previous.wait();
        }
    }
    // Probe before spawning: never attach to or terminate an unrelated process.
    let reservation = std::net::TcpListener::bind("127.0.0.1:11435")
        .map_err(|_| "Port 11435 is in use by another app. Close that app and try again.")?;
    drop(reservation);
    let mut command = Command::new(exe);
    command
        .arg("serve")
        .env("OLLAMA_HOST", "127.0.0.1:11435")
        .env("OLLAMA_MODELS", &directory)
        .env("OLLAMA_NO_CLOUD", "1")
        .env("OLLAMA_CONTEXT_LENGTH", "4096")
        .env("OLLAMA_MAX_LOADED_MODELS", "1")
        .env("OLLAMA_NUM_PARALLEL", "1")
        .env("OLLAMA_FLASH_ATTENTION", "1")
        .env("OLLAMA_KV_CACHE_TYPE", "q8_0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let child = command
        .spawn()
        .map_err(|_| "Could not start Ollama. Check the installed runtime.")?;
    *state.child.lock().unwrap() = Some(child);
    *state.directory.lock().unwrap() = directory.clone();
    for _ in 0..60 {
        if healthy().await {
            return status(state, &directory).await;
        }
        if state
            .child
            .lock()
            .unwrap()
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_some())
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err("Ollama did not become ready. Restart local AI and check that this runtime supports your models.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_manifests_are_not_offered_as_ready_models() {
        let dir = std::env::temp_dir().join(format!("momo-model-test-{}", std::process::id()));
        let library = dir.join("manifests/registry.ollama.ai/library/llama3.2");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(dir.join("blobs")).unwrap();
        let digest = format!("sha256:{}", "a".repeat(64));
        std::fs::write(
            library.join("3b"),
            serde_json::json!({"config":{"digest":digest},"layers":[]}).to_string(),
        )
        .unwrap();
        assert!(model_names(&dir).unwrap().is_empty());
        std::fs::write(dir.join("blobs").join(digest.replace(':', "-")), b"test").unwrap();
        assert_eq!(model_names(&dir).unwrap(), vec!["llama3.2:3b"]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
