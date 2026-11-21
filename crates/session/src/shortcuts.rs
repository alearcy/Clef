//! T18: the user's keyboard-shortcut changes, saved beside the audio preferences.
//!
//! The session stores them opaquely (command id → shortcut text, `null` for a cleared default);
//! the UI owns the command registry and interprets them.
use clef_model::Result;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read, path::Path};

const VERSION: u32 = 1;
const MAX_FILE: u64 = 64 * 1024;
const MAX_ENTRY: usize = 128;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    shortcuts: BTreeMap<String, Option<String>>,
}

/// Read the saved changes. A missing file means none; an unreadable or invalid file is
/// reported and treated as none, and is replaced on the next change.
pub(crate) fn read(path: &Path) -> (BTreeMap<String, Option<String>>, Option<String>) {
    let parsed = (|| -> Result<BTreeMap<String, Option<String>>> {
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(BTreeMap::new());
            }
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_FILE + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_FILE {
            return Err("file too large".into());
        }
        let file: File = serde_json::from_slice(&bytes)?;
        if file.version != VERSION {
            return Err("unsupported version".into());
        }
        validate(&file.shortcuts)?;
        Ok(file.shortcuts)
    })();
    match parsed {
        Ok(shortcuts) => (shortcuts, None),
        Err(error) => (
            BTreeMap::new(),
            Some(format!(
                "Keyboard shortcuts unreadable ({error}); using defaults."
            )),
        ),
    }
}

/// Atomically replace the saved changes.
pub(crate) fn write(path: &Path, shortcuts: &BTreeMap<String, Option<String>>) -> Result<()> {
    validate(shortcuts)?;
    let bytes = serde_json::to_vec_pretty(&File {
        version: VERSION,
        shortcuts: shortcuts.clone(),
    })?;
    crate::audio::write_atomic(path, &bytes)
}

fn validate(shortcuts: &BTreeMap<String, Option<String>>) -> Result<()> {
    let long = |text: &str| text.is_empty() || text.len() > MAX_ENTRY;
    if shortcuts.len() > 1024
        || shortcuts
            .iter()
            .any(|(id, value)| long(id) || value.as_deref().is_some_and(long))
    {
        return Err("invalid shortcut entries".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_shortcuts_round_trip_and_bad_files_fall_back() {
        let folder = std::env::temp_dir().join(format!(
            "clef-shortcuts-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = folder.join("shortcuts.json");
        assert_eq!(read(&path), (BTreeMap::new(), None));
        let saved = BTreeMap::from([
            ("file.save".to_owned(), None),
            ("transport.play".to_owned(), Some("Cmd+P".to_owned())),
        ]);
        write(&path, &saved).unwrap();
        assert_eq!(read(&path), (saved, None));
        std::fs::write(&path, b"{\"version\":2,\"shortcuts\":{}}").unwrap();
        let (shortcuts, error) = read(&path);
        assert!(shortcuts.is_empty() && error.unwrap().contains("unsupported version"));
        assert!(write(&path, &BTreeMap::from([(String::new(), None)])).is_err());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
