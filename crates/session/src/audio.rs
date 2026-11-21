//! T39: Clef's global audio preferences and the effective audio configuration.
//!
//! A project may override the global preferences. When the configured output device is
//! missing, an override falls back to the global device, and both fall back to the macOS
//! default output. A fallback never rewrites what is saved.
use clef_audio_io::Devices;
use clef_model::{Result, project::AudioSettings};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

const VERSION: u32 = 1;
const MAX_FILE: u64 = 64 * 1024;
/// Buffer used on a device whose saved configuration does not apply (first launch, fallback).
pub(crate) const DEFAULT_BUFFER: u32 = 128;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    audio: AudioSettings,
    /// Shown when the saved device is missing and its name cannot be enumerated.
    device_name: String,
}

#[derive(Default)]
pub(crate) struct Preferences {
    path: Option<PathBuf>,
    pub(crate) audio: Option<AudioSettings>,
    pub(crate) device_name: String,
}

impl Preferences {
    /// Read the preferences at `path`. An unreadable or invalid file is reported and treated
    /// as absent; it is left in place until the user applies settings.
    pub(crate) fn read(path: PathBuf) -> (Self, Option<String>) {
        let parsed = (|| -> Result<Option<File>> {
            let file = match std::fs::File::open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
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
            file.audio.validate()?;
            Ok(Some(file))
        })();
        match parsed {
            Ok(file) => (
                Self {
                    path: Some(path),
                    device_name: file
                        .as_ref()
                        .map(|f| f.device_name.clone())
                        .unwrap_or_default(),
                    audio: file.map(|f| f.audio),
                },
                None,
            ),
            Err(error) => (
                Self {
                    path: Some(path),
                    ..Self::default()
                },
                Some(format!(
                    "Audio preferences unreadable ({error}); using defaults."
                )),
            ),
        }
    }

    /// Atomically replace the saved preferences.
    pub(crate) fn write(&mut self, audio: AudioSettings, device_name: String) -> Result<()> {
        audio.validate()?;
        let path = self
            .path
            .as_deref()
            .ok_or("Clef's Application Support directory is unavailable.")?;
        let bytes = serde_json::to_vec_pretty(&File {
            version: VERSION,
            audio: audio.clone(),
            device_name: device_name.clone(),
        })?;
        write_atomic(path, &bytes)?;
        self.audio = Some(audio);
        self.device_name = device_name;
        Ok(())
    }
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or("invalid preferences path")?;
    std::fs::create_dir_all(parent)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("preferences");
    let temporary = parent.join(format!(".{stem}-{}-{nonce}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}

/// The configuration the engine starts with, and the notices the user must see.
#[derive(Debug, PartialEq)]
pub(crate) struct Resolved {
    pub(crate) settings: AudioSettings,
    pub(crate) notices: Vec<String>,
}

/// Resolve the effective configuration: the project override, else the global preferences,
/// else first-launch defaults on the macOS default output. Missing devices fall back as
/// agreed in T39; an `Err` means audio stays off and carries the message to show.
pub(crate) fn resolve(
    devices: &Devices,
    project: Option<&AudioSettings>,
    global: Option<&AudioSettings>,
    global_name: &str,
) -> std::result::Result<Resolved, String> {
    let present = |id: &str| devices.audio.iter().find(|device| device.id == id);
    let mut notices = Vec::new();
    let mut chosen = None;
    if let Some(project) = project {
        if present(&project.device).is_some() {
            chosen = Some(project.clone());
        } else {
            notices.push(format!(
                "This project's audio output ({}) is unavailable.",
                display(&project.device)
            ));
        }
    }
    if chosen.is_none()
        && let Some(global) = global
    {
        if present(&global.device).is_some() {
            chosen = Some(global.clone());
        } else if project.is_none_or(|project| project.device != global.device) {
            let name = if global_name.is_empty() {
                display(&global.device)
            } else {
                global_name.to_owned()
            };
            notices.push(format!("The saved audio output ({name}) is unavailable."));
        }
    }
    let settings = match chosen {
        Some(settings) => {
            devices
                .validate(&AudioSettings {
                    midi: String::new(),
                    ..settings.clone()
                })
                .map_err(|error| error.to_string())?;
            settings
        }
        None => {
            let base = project.or(global).cloned().unwrap_or_default();
            let fallback = devices
                .default_output
                .as_deref()
                .and_then(present)
                .ok_or_else(|| {
                    notices
                        .iter()
                        .cloned()
                        .chain(["No default audio output is available. Audio is off.".to_owned()])
                        .collect::<Vec<_>>()
                        .join(" ")
                })?;
            let mut settings = AudioSettings {
                device: fallback.id.clone(),
                ..base.clone()
            };
            if !fallback
                .configurations
                .iter()
                .any(|c| c.supports(&settings))
                || base.device.is_empty()
            {
                settings.rate = fallback.current_rate.unwrap_or(settings.rate);
                settings.buffer = DEFAULT_BUFFER;
                if !fallback
                    .configurations
                    .iter()
                    .any(|c| c.supports(&settings))
                {
                    settings.pair = [0, 1];
                }
            }
            if !fallback
                .configurations
                .iter()
                .any(|c| c.supports(&settings))
            {
                return Err(format!(
                    "The default audio output ({}) has no supported stereo configuration. Audio is off.",
                    fallback.name
                ));
            }
            if !notices.is_empty() {
                notices.push(format!(
                    "Using the macOS default output ({}).",
                    fallback.name
                ));
            }
            settings
        }
    };
    let mut settings = settings;
    if !settings.midi.is_empty() && !devices.midi.iter().any(|(id, _)| id == &settings.midi) {
        notices
            .push("The saved MIDI input is unavailable; audio started without MIDI input.".into());
        settings.midi.clear();
    }
    Ok(Resolved { settings, notices })
}

/// A readable form of a saved device identifier, for a device that cannot be enumerated.
fn display(id: &str) -> String {
    id.split(':')
        .nth(3)
        .filter(|name| !name.is_empty())
        .unwrap_or(id)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clef_audio_io::{AudioDevice, AudioRange, SupportedBufferSize};

    fn device(id: &str, name: &str, rate: u32) -> AudioDevice {
        AudioDevice {
            id: id.into(),
            name: name.into(),
            configurations: vec![AudioRange {
                channels: 2,
                min_rate: rate,
                max_rate: rate,
                buffer: SupportedBufferSize::Range { min: 16, max: 4096 },
            }],
            current_rate: Some(rate),
        }
    }
    fn settings(device: &str, rate: u32, buffer: u32) -> AudioSettings {
        AudioSettings {
            device: device.into(),
            pair: [0, 1],
            midi: "keys".into(),
            rate,
            buffer,
        }
    }
    fn devices(audio: Vec<AudioDevice>, default: Option<&str>) -> Devices {
        Devices {
            audio,
            midi: vec![("keys".into(), "Keys".into())],
            default_output: default.map(Into::into),
        }
    }

    #[test]
    fn override_then_global_then_first_launch_defaults() {
        let all = devices(
            vec![
                device("a", "A", 48_000),
                device("b", "B", 44_100),
                device("sys", "Mac", 44_100),
            ],
            Some("sys"),
        );
        let project = settings("a", 48_000, 64);
        let global = settings("b", 44_100, 256);
        let resolved = resolve(&all, Some(&project), Some(&global), "B").unwrap();
        assert_eq!((resolved.settings, resolved.notices.len()), (project, 0));
        let resolved = resolve(&all, None, Some(&global), "B").unwrap();
        assert_eq!((resolved.settings, resolved.notices.len()), (global, 0));
        let resolved = resolve(&all, None, None, "").unwrap();
        assert_eq!(
            resolved.settings,
            AudioSettings {
                device: "sys".into(),
                midi: String::new(),
                rate: 44_100,
                buffer: DEFAULT_BUFFER,
                ..AudioSettings::default()
            }
        );
        assert!(
            resolved.notices.is_empty(),
            "first launch is not a fallback"
        );
    }

    #[test]
    fn missing_override_tries_the_global_device_before_the_system_default() {
        let project = settings("coreaudio:x:Audient:Audient EVO16:1", 48_000, 64);
        let global = settings("b", 44_100, 256);
        let with_global = devices(
            vec![device("b", "B", 44_100), device("sys", "Mac", 48_000)],
            Some("sys"),
        );
        let resolved = resolve(&with_global, Some(&project), Some(&global), "B").unwrap();
        assert_eq!(resolved.settings, global);
        assert_eq!(
            resolved.notices,
            ["This project's audio output (Audient EVO16) is unavailable."]
        );

        let only_system = devices(vec![device("sys", "Mac", 48_000)], Some("sys"));
        let resolved = resolve(&only_system, Some(&project), Some(&global), "B").unwrap();
        assert_eq!(resolved.settings.device, "sys");
        assert_eq!(
            (resolved.settings.rate, resolved.settings.buffer),
            (48_000, 64),
            "supported values are kept"
        );
        assert_eq!(resolved.notices.len(), 3);
        assert!(resolved.notices[2].contains("Mac"));
    }

    #[test]
    fn fallback_adapts_unsupported_values_and_reports_no_output() {
        let global = settings("gone", 96_000, 32);
        let system = devices(vec![device("sys", "Mac", 44_100)], Some("sys"));
        let resolved = resolve(&system, None, Some(&global), "Studio").unwrap();
        assert_eq!(
            (resolved.settings.rate, resolved.settings.buffer),
            (44_100, DEFAULT_BUFFER)
        );
        assert_eq!(
            resolved.notices[0],
            "The saved audio output (Studio) is unavailable."
        );

        let none = devices(vec![], None);
        let error = resolve(&none, None, Some(&global), "Studio").unwrap_err();
        assert!(error.contains("Studio") && error.contains("Audio is off"));
        assert!(
            resolve(&none, None, None, "")
                .unwrap_err()
                .contains("Audio is off")
        );
    }

    #[test]
    fn missing_midi_input_starts_without_midi() {
        let global = AudioSettings {
            midi: "gone".into(),
            ..settings("a", 48_000, 128)
        };
        let resolved = resolve(
            &devices(vec![device("a", "A", 48_000)], None),
            None,
            Some(&global),
            "A",
        )
        .unwrap();
        assert!(resolved.settings.midi.is_empty());
        assert_eq!(resolved.notices.len(), 1);
    }

    #[test]
    fn preferences_round_trip_and_invalid_files_are_reported_but_kept() {
        let directory = clef_model::project::TemporaryDirectory::new("audio-preferences").unwrap();
        let path = directory.0.join("audio.json");
        let (mut preferences, error) = Preferences::read(path.clone());
        assert!(error.is_none() && preferences.audio.is_none());
        preferences
            .write(settings("a", 48_000, 128), "A".into())
            .unwrap();
        let (read, error) = Preferences::read(path.clone());
        assert!(error.is_none());
        assert_eq!(read.audio, Some(settings("a", 48_000, 128)));
        assert_eq!(read.device_name, "A");
        assert_eq!(
            std::fs::read_dir(&directory.0).unwrap().count(),
            1,
            "no temporary file left"
        );

        std::fs::write(&path, b"{not json").unwrap();
        let (read, error) = Preferences::read(path.clone());
        assert!(read.audio.is_none() && error.is_some());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"{not json",
            "an invalid file is not overwritten"
        );
    }
}
