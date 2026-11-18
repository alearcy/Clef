//! Persistent application-local plugin quarantine, independent of project files.
use crate::{catalog::Plugin, protocol::Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub plugin: Plugin,
    pub reason: String,
    pub time: u64,
}
pub fn same(a: &Plugin, b: &Plugin) -> bool {
    fn normalized(path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| {
            let mut result = PathBuf::new();
            for part in path.components() {
                match part {
                    std::path::Component::CurDir => {}
                    std::path::Component::ParentDir => {
                        result.pop();
                    }
                    other => result.push(other.as_os_str()),
                }
            }
            result
        })
    }
    a.id == b.id && a.format == b.format && normalized(&a.path) == normalized(&b.path)
}
pub fn location() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("HOME").ok_or("home directory unavailable")?)
            .join("Library/Application Support/Clef/quarantine.json"),
    )
}
pub fn read(path: &Path) -> Result<Vec<Entry>> {
    use std::io::Read;
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("quarantine file exceeds limit".into());
    }
    let entries: Vec<Entry> = serde_json::from_slice(&bytes)?;
    if entries.len() > 1024
        || entries.iter().any(|e| {
            e.reason.len() > 4096
                || e.plugin.id.is_empty()
                || e.plugin.id.len() > 4096
                || e.plugin.name.len() > 4096
                || !e.plugin.path.is_absolute()
                || e.plugin.path.as_os_str().len() > 16384
                || !matches!(e.plugin.format.as_str(), "VST3" | "CLAP")
        })
    {
        return Err("invalid quarantine records".into());
    }
    Ok(entries)
}
/// Merge under a separate stable lock inode; publication may replace the data inode.
pub fn update(path: &Path, plugin: &Plugin, reason: Option<&str>) -> Result<Vec<Entry>> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let parent = path.parent().ok_or("invalid quarantine path")?;
    std::fs::create_dir_all(parent)?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path.with_extension("lock"))?;
    lock.try_lock()
        .map_err(|e| format!("quarantine registry busy: {e}"))?;
    let mut entries = read(path)?;
    entries.retain(|entry| !same(&entry.plugin, plugin));
    if let Some(reason) = reason {
        entries.push(Entry {
            plugin: plugin.clone(),
            reason: reason.chars().take(1024).collect(),
            time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        });
    }
    let bytes = serde_json::to_vec(&entries)?;
    if entries.len() > 1024 || bytes.len() > 1024 * 1024 {
        return Err("quarantine registry full".into());
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let tmp = parent.join(format!(".quarantine-{}-{nonce}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(tmp);
    result?;
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plugin(id: &str) -> Plugin {
        Plugin {
            path: "/missing/Test.vst3".into(),
            id: id.into(),
            name: "Test".into(),
            format: "VST3".into(),
        }
    }
    #[test]
    fn quarantine_survives_reopen_and_keeps_other_entries_and_formats_independent() {
        let d = crate::project::TemporaryDirectory::new("quarantine").unwrap();
        let path = d.0.join("registry.json");
        update(&path, &plugin("a"), Some("crash")).unwrap();
        update(&path, &plugin("b"), Some("timeout")).unwrap();
        assert_eq!(read(&path).unwrap().len(), 2);
        let entries = update(&path, &plugin("a"), None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].plugin.id, "b");
        let mut other = plugin("b");
        other.format = "CLAP".into();
        assert!(!same(&entries[0].plugin, &other));
    }
    #[test]
    fn corruption_and_write_failure_do_not_clear_records() {
        let d = crate::project::TemporaryDirectory::new("quarantine-bad").unwrap();
        let path = d.0.join("registry.json");
        std::fs::write(&path, b"invalid").unwrap();
        assert!(read(&path).is_err());
        assert!(update(&path, &plugin("a"), None).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"invalid");
        assert!(update(&d.0, &plugin("a"), Some("crash")).is_err());
    }
}
