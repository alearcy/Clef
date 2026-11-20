use crate::protocol::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub use clef_model::Plugin;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub plugins: Vec<Plugin>,
    pub effects: Vec<Plugin>,
    pub failures: Vec<String>,
}

unsafe extern "C" {
    fn clef_vst_scan(
        path: *const c_char,
        include_effects: i32,
        entry: extern "C" fn(*const c_char, *const c_char, i32, *mut c_void),
        context: *mut c_void,
    ) -> i32;
    fn clef_vst_error() -> *const c_char;
}

// This function runs only in a disposable scanner process, never in the application.
pub fn inspect(path: &Path, output: &Path, categorized: bool) -> Result<()> {
    let mut report = Catalog::default();
    match path.extension().and_then(|s| s.to_str()) {
        Some("vst3") => {
            extern "C" fn entry(
                id: *const c_char,
                name: *const c_char,
                effect: i32,
                context: *mut c_void,
            ) {
                let entries = unsafe { &mut *context.cast::<Vec<(String, String, bool)>>() };
                if entries.len() < 4096 {
                    let text = |p| {
                        unsafe { CStr::from_ptr(p) }
                            .to_string_lossy()
                            .chars()
                            .take(1024)
                            .collect()
                    };
                    entries.push((text(id), text(name), effect != 0));
                }
            }
            use std::os::unix::ffi::OsStrExt;
            let file = CString::new(path.as_os_str().as_bytes())?;
            let mut entries = Vec::<(String, String, bool)>::new();
            if unsafe {
                clef_vst_scan(
                    file.as_ptr(),
                    categorized.into(),
                    entry,
                    (&mut entries as *mut Vec<_>).cast(),
                )
            } != 1
            {
                return Err(unsafe { CStr::from_ptr(clef_vst_error()) }
                    .to_string_lossy()
                    .into_owned()
                    .into());
            }
            for (id, name, effect) in entries {
                let list = if effect {
                    &mut report.effects
                } else {
                    &mut report.plugins
                };
                list.push(Plugin {
                    path: path.into(),
                    id,
                    name,
                    format: "VST3".into(),
                });
            }
        }
        Some("clap") => {
            let entry =
                unsafe { clack_host::entry::PluginEntry::load(path) }.map_err(|e| e.to_string())?;
            let factory = entry
                .get_plugin_factory()
                .ok_or("CLAP factory unavailable")?;
            for descriptor in factory.plugin_descriptors().take(4096) {
                let instrument = descriptor.features().any(|f| f.to_bytes() == b"instrument");
                let effect = categorized
                    && descriptor
                        .features()
                        .any(|f| f.to_bytes() == b"audio-effect");
                if !instrument && !effect {
                    continue;
                }
                let id = descriptor
                    .id()
                    .ok_or("CLAP ID missing")?
                    .to_str()?
                    .to_owned();
                let name = descriptor
                    .name()
                    .ok_or("CLAP name missing")?
                    .to_string_lossy()
                    .into_owned();
                let plugin = Plugin {
                    path: path.into(),
                    id,
                    name,
                    format: "CLAP".into(),
                };
                if effect
                    && crate::clap_host::ClapMain::load(
                        path,
                        &plugin.id,
                        crate::protocol::RATE,
                        true,
                    )
                    .is_ok()
                {
                    report.effects.push(plugin.clone());
                }
                if instrument {
                    report.plugins.push(plugin);
                }
            }
        }
        _ => return Err("unsupported plugin bundle".into()),
    }
    let bytes = if categorized {
        serde_json::to_vec(&report)?
    } else {
        serde_json::to_vec(&report.plugins)?
    };
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("plugin descriptor report too large".into());
    }
    crate::files::save_state(output, &bytes)
}

fn bundles(
    root: &Path,
    seen: &mut HashSet<PathBuf>,
    found: &mut Vec<PathBuf>,
    failures: &mut Vec<String>,
    depth: usize,
) {
    if depth > 16 || found.len() >= 16384 {
        return;
    }
    let Ok(root) = root.canonicalize() else {
        return;
    };
    if !seen.insert(root.clone()) {
        return;
    }
    if root.extension().is_some_and(|s| s == "vst3" || s == "clap") {
        found.push(root);
        return;
    }
    if !root.is_dir() {
        return;
    }
    match std::fs::read_dir(&root) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => bundles(&entry.path(), seen, found, failures, depth + 1),
                    Err(e) => failures.push(e.to_string()),
                }
            }
        }
        Err(e) => failures.push(format!("{}: {e}", root.display())),
    }
}

fn scan(executable: &Path, cancel: &std::sync::atomic::AtomicBool) -> Catalog {
    let mut report = Catalog::default();
    let mut roots = vec![
        PathBuf::from("/Library/Audio/Plug-Ins/VST3"),
        PathBuf::from("/Library/Audio/Plug-Ins/CLAP"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(&home).join("Library/Audio/Plug-Ins/VST3"));
        roots.push(PathBuf::from(home).join("Library/Audio/Plug-Ins/CLAP"));
    }
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        if root.exists() {
            bundles(&root, &mut seen, &mut found, &mut report.failures, 0);
        }
    }
    found.sort();
    let quarantine = match crate::quarantine::location().and_then(|p| crate::quarantine::read(&p)) {
        Ok(entries) => entries,
        Err(e) => {
            report
                .failures
                .push(format!("Quarantine unavailable; scanning blocked: {e}"));
            return report;
        }
    };
    for path in found {
        if quarantine.iter().any(|entry| {
            entry
                .plugin
                .path
                .canonicalize()
                .unwrap_or_else(|_| entry.plugin.path.clone())
                == path
        }) {
            for entry in quarantine.iter().filter(|entry| {
                entry
                    .plugin
                    .path
                    .canonicalize()
                    .unwrap_or_else(|_| entry.plugin.path.clone())
                    == path
            }) {
                let list = if entry.effect {
                    &mut report.effects
                } else {
                    &mut report.plugins
                };
                list.push(entry.plugin.clone());
            }
            report
                .failures
                .push(format!("{}: quarantined bundle skipped", path.display()));
            continue;
        }
        if cancel.load(std::sync::atomic::Ordering::Acquire) {
            break;
        }
        match scan_bundle_cancellable(executable, &path, Duration::from_secs(15), cancel) {
            Ok(entries) => {
                report.plugins.extend(entries.plugins);
                report.effects.extend(entries.effects);
            }
            Err(e) => report.failures.push(format!("{}: {e}", path.display())),
        }
    }
    for list in [&mut report.plugins, &mut report.effects] {
        list.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.format.cmp(&b.format))
        });
    }
    report
}

fn scan_bundle_cancellable(
    executable: &Path,
    path: &Path,
    timeout: Duration,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Catalog> {
    let directory = crate::project::TemporaryDirectory::new("scan")?;
    let output = directory.0.join("report.json");
    let mut child = Command::new(executable)
        .arg("scan-bundle-categorized")
        .arg(path)
        .arg(&output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    wait_for_scan(&mut child, timeout, cancel)?;
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(&output)?
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("scan report exceeds limit".into());
    }
    let entries: Catalog = serde_json::from_slice(&bytes)?;
    if entries.plugins.len() > 4096
        || entries.effects.len() > 4096
        || !entries.failures.is_empty()
        || entries.plugins.iter().chain(&entries.effects).any(|p| {
            p.path != path
                || p.id.is_empty()
                || p.id.len() > 4096
                || p.name.len() > 4096
                || !matches!(p.format.as_str(), "VST3" | "CLAP")
        })
    {
        return Err("invalid scan report".into());
    }
    Ok(entries)
}

fn wait_for_scan(
    child: &mut std::process::Child,
    timeout: Duration,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(format!("inspection failed ({status}); use Rescan to retry").into());
                }
                return Ok(());
            }
            Ok(None)
                if Instant::now() < deadline
                    && !cancel.load(std::sync::atomic::Ordering::Acquire) =>
            {
                std::thread::sleep(Duration::from_millis(10))
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match result {
                    Err(e) => e.into(),
                    _ => "inspection timed out; use Rescan to retry".into(),
                });
            }
        }
    }
}

pub struct ScanJob {
    pub receive: std::sync::mpsc::Receiver<Catalog>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl ScanJob {
    pub fn spawn(executable: PathBuf, notify: impl FnOnce() + Send + 'static) -> Self {
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stopping = cancel.clone();
        let (send, receive) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let _ = send.send(scan(&executable, &stopping));
            notify();
        });
        Self {
            receive,
            cancel,
            thread: Some(thread),
        }
    }
}
impl Drop for ScanJob {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_deduplicates_symlinks_and_does_not_descend_into_bundles() {
        let directory = crate::project::TemporaryDirectory::new("discovery-test").unwrap();
        let plugin = directory.0.join("Synth.vst3");
        std::fs::create_dir(&plugin).unwrap();
        std::fs::create_dir(plugin.join("Internal.clap")).unwrap();
        std::fs::write(directory.0.join("support.txt"), b"not a plugin").unwrap();
        std::os::unix::fs::symlink(&directory.0, directory.0.join("cycle")).unwrap();
        let mut found = Vec::new();
        let mut failures = Vec::new();
        bundles(
            &directory.0,
            &mut HashSet::new(),
            &mut found,
            &mut failures,
            0,
        );
        assert!(failures.is_empty());
        assert_eq!(found, [plugin.canonicalize().unwrap()]);
    }
    #[test]
    fn timed_out_and_cancelled_scanners_are_killed_and_reaped() {
        use std::sync::atomic::AtomicBool;
        for cancel in [false, true] {
            let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
            let started = Instant::now();
            assert!(
                wait_for_scan(
                    &mut child,
                    Duration::from_millis(30),
                    &AtomicBool::new(cancel)
                )
                .is_err()
            );
            assert!(started.elapsed() < Duration::from_secs(2));
            assert!(child.try_wait().unwrap().is_some());
        }
    }
}
