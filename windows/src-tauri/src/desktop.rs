//! Small, explicit desktop tools. Model output never becomes a shell command.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub name: String,
    pub detail: String,
    pub success: bool,
    pub path: Option<String>,
}

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"name":"open_app","description":"Open an application requested by the user. Only the listed apps are available. Do not claim success until the tool confirms it.","parameters":{"type":"object","properties":{"app":{"type":"string","enum":["notepad","calculator","paint","file_explorer"]}},"required":["app"],"additionalProperties":false}}),
        json!({"name":"create_note","description":"Create a new UTF-8 text note in the MoMo notes folder and open it in Notepad (the default text editor on Linux). Use only when the user asks to create a note. Existing files are never overwritten.","parameters":{"type":"object","properties":{"title":{"type":"string","description":"Short note title; not a file path."},"content":{"type":"string","description":"Full note text in the user's language."}},"required":["title","content"],"additionalProperties":false}}),
    ]
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AppArgs {
    app: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NoteArgs {
    title: String,
    content: String,
}

pub fn fingerprint(name: &str, args: &Value) -> String {
    let normalized = match name {
        "open_app" => serde_json::from_value::<AppArgs>(args.clone())
            .ok()
            .and_then(|a| serde_json::to_string(&a).ok()),
        "create_note" => serde_json::from_value::<NoteArgs>(args.clone())
            .ok()
            .and_then(|a| serde_json::to_string(&a).ok()),
        _ => None,
    };
    format!("{name}:{}", normalized.unwrap_or_else(|| args.to_string()))
}

pub fn execute(name: &str, args: &Value) -> Action {
    let result = match name {
        "open_app" => serde_json::from_value::<AppArgs>(args.clone())
            .map_err(|_| "Invalid app arguments.".to_string())
            .and_then(|args| {
                open_app(&args.app)
                    .map(|_| (format!("Opened {}", args.app.replace('_', " ")), None))
            }),
        "create_note" => serde_json::from_value::<NoteArgs>(args.clone())
            .map_err(|_| "Invalid note arguments.".to_string())
            .and_then(|args| {
                let path = write_note(
                    &crate::settings::local_dir().join("notes"),
                    &args.title,
                    &args.content,
                )?;
                let display = path.to_string_lossy().to_string();
                match open_note(&path) {
                    Ok(()) => Ok((
                        format!("Saved and opened note: {}", args.title),
                        Some(display),
                    )),
                    Err(error) => Ok((
                        format!("Note saved; editor could not open: {error}"),
                        Some(display),
                    )),
                }
            }),
        _ => Err("This desktop action is not supported.".into()),
    };
    match result {
        Ok((detail, path)) => Action {
            name: name.into(),
            detail,
            success: true,
            path,
        },
        Err(detail) => Action {
            name: name.into(),
            detail,
            success: false,
            path: None,
        },
    }
}

fn write_note(directory: &Path, title: &str, content: &str) -> Result<PathBuf, String> {
    if title.trim().is_empty()
        || title.chars().count() > 80
        || content.is_empty()
        || content.len() > 128_000
    {
        return Err("Notes need a title of 1–80 characters and text of at most 128 KB.".into());
    }
    let stem: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    crate::platform::ensure_private_dir(directory)
        .map_err(|_| "Cannot create the notes folder.")?;
    for index in 1..=10000 {
        let path = directory.join(format!("note-{stem}-{index}.txt"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(content.as_bytes())
                    .and_then(|_| file.sync_all())
                    .map_err(|_| "Could not save the note.")?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err("Cannot create this note file.".into()),
        }
    }
    Err("Too many notes have this title. Choose another title.".into())
}

#[cfg(windows)]
fn system_app(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()))
        .join("System32")
        .join(name)
}

#[cfg(windows)]
fn open_app(app: &str) -> Result<(), String> {
    let executable = match app {
        "notepad" => "notepad.exe",
        "calculator" => "calc.exe",
        "paint" => "mspaint.exe",
        "file_explorer" => "explorer.exe",
        _ => return Err("Choose Notepad, Calculator, Paint or File Explorer.".into()),
    };
    let path = if app == "file_explorer" {
        system_app("../explorer.exe")
    } else {
        system_app(executable)
    };
    Command::new(path)
        .spawn()
        .map(|_| ())
        .map_err(|_| format!("Could not open {app}. Check that it is installed."))
}

#[cfg(windows)]
fn open_note(path: &Path) -> Result<(), String> {
    Command::new(system_app("notepad.exe"))
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|_| "Notepad is unavailable.".into())
}

#[cfg(not(windows))]
fn open_note(path: &Path) -> Result<(), String> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|_| "No default text editor is available.".into())
}

#[cfg(not(windows))]
fn open_app(app: &str) -> Result<(), String> {
    let names: &[&str] = match app {
        "notepad" => &["gedit", "kate", "mousepad", "xed"],
        "calculator" => &["gnome-calculator", "kcalc", "galculator"],
        "paint" => &["pinta", "kolourpaint"],
        "file_explorer" => &["xdg-open"],
        _ => return Err("Unsupported application.".into()),
    };
    for name in names {
        let mut command = Command::new(name);
        if app == "file_explorer" {
            command.arg(".");
        }
        if command.spawn().is_ok() {
            return Ok(());
        }
    }
    Err(format!(
        "No supported app found for {app}. Install a default editor or application first."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notes_stay_in_their_folder_and_never_overwrite_existing_text() {
        let dir = std::env::temp_dir().join(format!("momo-note-test-{}", std::process::id()));
        let first = write_note(&dir, "../CON/meeting", "first\n日本語").unwrap();
        let second = write_note(&dir, "../CON/meeting", "second").unwrap();
        assert_eq!(first.parent(), Some(dir.as_path()));
        assert_ne!(first, second);
        assert_eq!(std::fs::read_to_string(first).unwrap(), "first\n日本語");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn arbitrary_commands_and_extra_arguments_are_rejected() {
        assert!(!execute("open_app", &json!({"app":"powershell"})).success);
        assert!(
            !execute(
                "open_app",
                &json!({"app":"notepad","arguments":"secret.txt"})
            )
            .success
        );
        assert!(!execute("shell", &json!({"command":"delete"})).success);
    }
}
