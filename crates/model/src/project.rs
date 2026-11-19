use crate::{DEFAULT_BLOCK_SIZE as BLOCK, DEFAULT_SAMPLE_RATE as RATE, MAX_STATE, Plugin, Result};
use crate::{
    midi::{Arrangement, Snap},
    timeline::{MAX_TICK, Timeline},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

const MAGIC: &[u8; 8] = b"CLEFPRJ2";
const MAX_METADATA: usize = 16 * 1024 * 1024;
pub const MAX_TRACKS: usize = 128;
const MAX_TOTAL_STATE: usize = 512 * 1024 * 1024;

pub struct TemporaryDirectory(pub PathBuf);
impl TemporaryDirectory {
    pub fn new(label: &str) -> Result<Self> {
        use std::os::unix::fs::DirBuilderExt;
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "clef-{label}-{}-{nonce}-{serial}",
            std::process::id()
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSettings {
    pub device: String,
    pub pair: [usize; 2],
    pub midi: String,
    pub rate: u32,
    pub buffer: u32,
}
impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            device: String::new(),
            pair: [0, 1],
            midi: String::new(),
            rate: RATE,
            buffer: BLOCK as u32,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub id: u64,
    pub name: String,
    pub plugin: Option<Plugin>,
    #[serde(default, skip_serializing_if = "crate::mixer::Channel::is_default")]
    pub mix: crate::mixer::Channel,
    #[serde(skip)]
    pub state: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub version: u32,
    pub tracks: Vec<Track>,
    pub next_id: u64,
    pub audio: AudioSettings,
    #[serde(default)]
    pub timeline: Timeline,
    #[serde(default)]
    pub midi: Arc<Arrangement>,
    #[serde(default)]
    pub arrangement_view: ArrangementView,
    #[serde(default)]
    pub master: crate::mixer::Level,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrangementView {
    pub snap: Snap,
    pub offset: u64,
    pub zoom_milli: u32,
}

impl Default for ArrangementView {
    fn default() -> Self {
        Self {
            snap: Snap::Beat,
            offset: 0,
            zoom_milli: 48_000,
        }
    }
}

impl ArrangementView {
    pub fn validate(self) -> Result<()> {
        if self.offset > MAX_TICK || !(8_000..=240_000).contains(&self.zoom_milli) {
            return Err("Arrangement viewport is outside its supported range.".into());
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    version: u32,
    tracks: Vec<Track>,
    next_id: u64,
    audio: AudioSettings,
    #[serde(default)]
    timeline: Timeline,
    #[serde(default, skip_serializing_if = "Arrangement::is_default")]
    midi: Arc<Arrangement>,
    #[serde(default, skip_serializing_if = "default_arrangement_view")]
    arrangement_view: ArrangementView,
    #[serde(default, skip_serializing_if = "crate::mixer::Level::is_default")]
    master: crate::mixer::Level,
    state_lengths: Vec<usize>,
}

fn default_arrangement_view(view: &ArrangementView) -> bool {
    *view == ArrangementView::default()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyTrack {
    name: String,
    plugin: Option<Plugin>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyProject {
    version: u32,
    track: Option<LegacyTrack>,
    audio: AudioSettings,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            version: 2,
            tracks: Vec::new(),
            next_id: 1,
            audio: AudioSettings::default(),
            timeline: Timeline::default(),
            midi: Arc::default(),
            arrangement_view: ArrangementView::default(),
            master: Default::default(),
        }
    }
}

impl Project {
    pub fn mix(&self, target: crate::mixer::Target) -> Result<crate::mixer::Channel> {
        match target {
            crate::mixer::Target::Master => Ok(crate::mixer::Channel {
                level: self.master,
                ..Default::default()
            }),
            crate::mixer::Target::Track(id) => self
                .tracks
                .iter()
                .find(|track| track.id == id)
                .map(|track| track.mix)
                .ok_or_else(|| "Mixer track is unavailable.".into()),
        }
    }

    pub fn set_mix(
        &mut self,
        target: crate::mixer::Target,
        control: crate::mixer::Control,
        value: i16,
    ) -> Result<()> {
        if !target.accepts(control) {
            return Err("Mixer control is unavailable for this channel.".into());
        }
        match target {
            crate::mixer::Target::Master => {
                let mut channel = self.mix(target)?;
                channel.set(control, value)?;
                self.master = channel.level;
                Ok(())
            }
            crate::mixer::Target::Track(id) => self
                .tracks
                .iter_mut()
                .find(|track| track.id == id)
                .ok_or("Mixer track is unavailable.")?
                .mix
                .set(control, value),
        }
    }

    pub fn validate(&self) -> Result<()> {
        self.timeline.validate()?;
        if !self.mix(crate::mixer::Target::Master)?.valid() {
            return Err("Invalid master mixer settings.".into());
        }
        self.arrangement_view.validate()?;
        self.midi
            .validate(self.tracks.iter().map(|track| track.id))?;
        let valid_text = |text: &str| text.len() <= 4096 && !text.contains('\0');
        if self.version != 2 {
            return Err("unsupported Clef project version".into());
        }
        if !(8000..=384000).contains(&self.audio.rate)
            || !(1..=65536).contains(&self.audio.buffer)
            || self.audio.pair[0] == self.audio.pair[1]
            || self.audio.pair.iter().any(|n| *n >= 4096)
            || !valid_text(&self.audio.device)
            || !valid_text(&self.audio.midi)
        {
            return Err("invalid project audio configuration".into());
        }
        if self.tracks.len() > MAX_TRACKS || self.next_id == 0 {
            return Err("invalid track count or next identity".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut total = 0usize;
        for track in &self.tracks {
            if track.id == 0 || track.id >= self.next_id || !ids.insert(track.id) {
                return Err("invalid or duplicate track identity".into());
            }
            total = total
                .checked_add(track.state.len())
                .ok_or("state size overflow")?;
            if total > MAX_TOTAL_STATE {
                return Err("combined state exceeds limit".into());
            }
            if !valid_text(&track.name)
                || track.name.trim().is_empty()
                || track.state.len() > MAX_STATE
            {
                return Err("invalid track name or state size".into());
            }
            if !track.mix.valid() {
                return Err("Invalid track mixer settings.".into());
            }
            if let Some(plugin) = &track.plugin {
                if !valid_text(&plugin.name)
                    || !valid_text(&plugin.id)
                    || plugin.id.is_empty()
                    || !plugin.path.is_absolute()
                    || plugin.path.as_os_str().len() > 16384
                    || !matches!(
                        (
                            plugin.format.as_str(),
                            plugin.path.extension().and_then(|s| s.to_str())
                        ),
                        ("VST3", Some("vst3")) | ("CLAP", Some("clap"))
                    )
                {
                    return Err("invalid plugin identity".into());
                }
            } else if !track.state.is_empty() {
                return Err("state has no associated plugin".into());
            }
        }
        Ok(())
    }
    fn write(&self, writer: &mut impl Write) -> Result<()> {
        self.validate()?;
        let lengths: Vec<usize> = self.tracks.iter().map(|t| t.state.len()).collect();
        let metadata = serde_json::to_vec(&Metadata {
            version: self.version,
            tracks: self
                .tracks
                .iter()
                .map(|t| Track {
                    id: t.id,
                    name: t.name.clone(),
                    plugin: t.plugin.clone(),
                    mix: t.mix,
                    state: Vec::new(),
                })
                .collect(),
            next_id: self.next_id,
            audio: self.audio.clone(),
            timeline: self.timeline.clone(),
            midi: self.midi.clone(),
            arrangement_view: self.arrangement_view,
            master: self.master,
            state_lengths: lengths.clone(),
        })?;
        if metadata.len() > MAX_METADATA {
            return Err("project metadata exceeds limit".into());
        }
        let total: usize = lengths.iter().sum();
        writer.write_all(MAGIC)?;
        writer.write_all(&(metadata.len() as u32).to_le_bytes())?;
        writer.write_all(&(total as u32).to_le_bytes())?;
        writer.write_all(&metadata)?;
        for track in &self.tracks {
            writer.write_all(&track.state)?;
        }
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self> {
        Self::decode(&mut File::open(path)?)
    }
    fn decode(reader: &mut impl Read) -> Result<Self> {
        let mut header = [0; 16];
        reader.read_exact(&mut header)?;
        let v1 = &header[..8] == b"CLEFPRJ1";
        if !v1 && &header[..8] != MAGIC {
            return Err("not a supported Clef project".into());
        }
        let metadata_size = u32::from_le_bytes(header[8..12].try_into()?) as usize;
        let total = u32::from_le_bytes(header[12..16].try_into()?) as usize;
        if metadata_size == 0
            || metadata_size > if v1 { 65536 } else { MAX_METADATA }
            || total > if v1 { MAX_STATE } else { MAX_TOTAL_STATE }
        {
            return Err("project exceeds size limits".into());
        }
        let mut metadata = vec![0; metadata_size];
        reader.read_exact(&mut metadata)?;
        let (mut project, lengths) = if v1 {
            let legacy: LegacyProject = serde_json::from_slice(&metadata)?;
            if legacy.version != 1 {
                return Err("invalid v1 project version".into());
            }
            let tracks: Vec<_> = legacy
                .track
                .into_iter()
                .map(|t| Track {
                    id: 1,
                    name: t.name,
                    plugin: t.plugin,
                    mix: Default::default(),
                    state: Vec::new(),
                })
                .collect();
            if tracks.is_empty() && total != 0 {
                return Err("orphan v1 plugin state".into());
            }
            let lengths = if tracks.is_empty() {
                vec![]
            } else {
                vec![total]
            };
            (
                Self {
                    version: 2,
                    next_id: tracks.len() as u64 + 1,
                    tracks,
                    audio: legacy.audio,
                    timeline: Timeline::default(),
                    midi: Arc::default(),
                    arrangement_view: ArrangementView::default(),
                    master: Default::default(),
                },
                lengths,
            )
        } else {
            let metadata: Metadata = serde_json::from_slice(&metadata)?;
            (
                Self {
                    version: metadata.version,
                    tracks: metadata.tracks,
                    next_id: metadata.next_id,
                    audio: metadata.audio,
                    timeline: metadata.timeline,
                    midi: metadata.midi,
                    arrangement_view: metadata.arrangement_view,
                    master: metadata.master,
                },
                metadata.state_lengths,
            )
        };
        project.validate()?;
        if lengths.len() != project.tracks.len()
            || lengths.iter().any(|n| *n > MAX_STATE)
            || lengths
                .iter()
                .try_fold(0usize, |sum, n| sum.checked_add(*n))
                != Some(total)
        {
            return Err("invalid project state lengths".into());
        }
        for (track, length) in project.tracks.iter_mut().zip(lengths) {
            if length != 0 && track.plugin.is_none() {
                return Err("orphan plugin state".into());
            }
            track.state = vec![0; length];
            reader.read_exact(&mut track.state)?;
        }
        let mut extra = [0];
        if reader.read(&mut extra)? != 0 {
            return Err("unexpected trailing project data".into());
        }
        project.validate()?;
        Ok(project)
    }
    pub fn save(&self, path: &Path, replace: bool) -> Result<()> {
        self.validate()?;
        atomic_write(path, replace, |file| self.write(file))
    }
}

/// Publish a synced file from a temporary sibling, optionally replacing the destination.
pub fn atomic_write(
    path: &Path,
    replace: bool,
    write: impl FnOnce(&mut File) -> Result<()>,
) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temporary = parent.join(format!(".clef-save-{}-{nonce}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    let result = (|| {
        write(&mut file)?;
        file.sync_all()?;
        if replace {
            std::fs::rename(&temporary, path)?;
        } else {
            std::fs::hard_link(&temporary, path)?;
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(temporary);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Project {
        Project {
            tracks: vec![Track {
                id: 1,
                name: "Piano".into(),
                plugin: Some(Plugin {
                    path: "/Library/Audio/Plug-Ins/VST3/Piano.vst3".into(),
                    id: "piano".into(),
                    name: "Piano".into(),
                    format: "VST3".into(),
                }),
                mix: Default::default(),
                state: vec![0, 255, 1, 2],
            }],
            next_id: 2,
            ..Default::default()
        }
    }

    #[test]
    fn mixer_round_trips_and_older_metadata_opens_at_unity() {
        use crate::mixer::{Channel, Control, Level, Target};
        let mut project = sample();
        let mut bytes = Vec::new();
        project.write(&mut bytes).unwrap();
        let header = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let metadata: serde_json::Value = serde_json::from_slice(&bytes[16..16 + header]).unwrap();
        assert!(metadata.get("master").is_none());
        assert!(metadata["tracks"][0].get("mix").is_none());
        let old = Project::decode(&mut bytes.as_slice()).unwrap();
        assert_eq!(old.master, Level::default());
        assert_eq!(old.tracks[0].mix, Channel::default());

        for (control, value) in [
            (Control::Gain, 91),
            (Control::Pan, -52),
            (Control::Width, 178),
            (Control::Fader, -107),
            (Control::Mute, 1),
            (Control::Solo, 1),
        ] {
            project.set_mix(Target::Track(1), control, value).unwrap();
        }
        project.set_mix(Target::Master, Control::Pan, 20).unwrap();
        project
            .set_mix(Target::Master, Control::Fader, -45)
            .unwrap();
        bytes.clear();
        project.write(&mut bytes).unwrap();
        assert_eq!(Project::decode(&mut bytes.as_slice()).unwrap(), project);
        assert!(project.set_mix(Target::Master, Control::Width, 42).is_err());
        project.tracks[0].mix.width = 201;
        assert!(project.write(&mut bytes).is_err());
    }
    #[test]
    fn project_round_trip_keeps_binary_state_and_routing() {
        let mut bytes = Vec::new();
        let project = sample();
        project.write(&mut bytes).unwrap();
        assert_eq!(Project::decode(&mut bytes.as_slice()).unwrap(), project);
        bytes.push(1);
        assert!(Project::decode(&mut bytes.as_slice()).is_err());
    }
    #[test]
    fn timeline_options_round_trip_and_older_v2_projects_get_defaults() {
        use crate::timeline::{StopMode, Tempo};
        let mut project = sample();
        project.timeline.changes.push(Tempo {
            bar: 9,
            bpm: 107,
            beats: 7,
            unit: 8,
        });
        project.timeline.loop_range = [1200, 10_500];
        project.timeline.looping = true;
        project.timeline.click = true;
        project.timeline.count_in = true;
        project.timeline.stop_mode = StopMode::ReturnToPlayStart;
        let mut bytes = Vec::new();
        project.write(&mut bytes).unwrap();
        assert_eq!(Project::decode(&mut bytes.as_slice()).unwrap(), project);
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&bytes[16..16 + length]).unwrap();
        metadata.as_object_mut().unwrap().remove("timeline");
        let original = container(
            MAGIC,
            &serde_json::to_vec(&metadata).unwrap(),
            &bytes[16 + length..],
        );
        let legacy = Project::decode(&mut original.as_slice()).unwrap();
        assert_eq!(legacy.timeline, Timeline::default());
        assert_eq!(legacy.tracks, project.tracks);
        project.timeline.changes[1].bar = 1;
        assert!(project.validate().is_err());
        project.timeline.changes.pop();
        project.timeline.loop_range = [15, 14];
        assert!(project.validate().is_err());
    }
    #[test]
    fn midi_modes_takes_and_sections_round_trip_and_remain_optional_in_old_v2() {
        use crate::midi::{Event, Message, Mode};
        let mut project = sample();
        let track = project.tracks[0].id;
        let midi = Arc::make_mut(&mut project.midi);
        midi.mode = Mode::NewTake;
        midi.record(
            track,
            Mode::NewTake,
            [0, 240],
            (0..2)
                .map(|value| {
                    vec![Event {
                        tick: 100,
                        order: 0,
                        message: Message::Control([0xb1, 1, value]),
                    }]
                })
                .collect(),
            false,
        )
        .unwrap();
        let mut bytes = Vec::new();
        project.write(&mut bytes).unwrap();
        assert_eq!(Project::decode(&mut bytes.as_slice()).unwrap(), project);
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&bytes[16..16 + length]).unwrap();
        metadata.as_object_mut().unwrap().remove("midi").unwrap();
        let old = container(
            MAGIC,
            &serde_json::to_vec(&metadata).unwrap(),
            &bytes[16 + length..],
        );
        assert!(
            Project::decode(&mut old.as_slice())
                .unwrap()
                .midi
                .is_default()
        );
    }

    #[test]
    fn oversized_truncated_and_future_projects_are_rejected() {
        let mut bytes = Vec::new();
        sample().write(&mut bytes).unwrap();
        assert!(Project::decode(&mut &bytes[..bytes.len() - 1]).is_err());
        bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Project::decode(&mut bytes.as_slice()).is_err());
        let mut project = sample();
        project.version = 3;
        assert!(project.write(&mut Vec::new()).is_err());
    }
    #[test]
    fn failed_save_preserves_the_previous_project_and_cleans_temporary_file() {
        let directory = TemporaryDirectory::new("save-test").unwrap();
        let path = directory.0.join("Music.clef");
        let project = sample();
        project.save(&path, false).unwrap();
        let mut changed = project.clone();
        changed.tracks.first_mut().unwrap().name = "Changed".into();
        assert!(changed.save(&path, false).is_err());
        assert_eq!(Project::read(&path).unwrap(), project);
        changed.version = 3;
        assert!(changed.save(&path, true).is_err());
        assert_eq!(Project::read(&path).unwrap(), project);
        changed.version = 2;
        changed.save(&path, true).unwrap();
        assert_eq!(Project::read(&path).unwrap(), changed);
        assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
    }
    #[test]
    fn partial_writer_failure_does_not_succeed() {
        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("disk full"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(sample().write(&mut FailingWriter).is_err());
    }
    fn container(magic: &[u8; 8], metadata: &[u8], payload: &[u8]) -> Vec<u8> {
        let mut bytes = magic.to_vec();
        bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(metadata);
        bytes.extend_from_slice(payload);
        bytes
    }
    #[test]
    fn legacy_migration_preserves_all_state_and_never_modifies_the_source() {
        let directory = TemporaryDirectory::new("v1-migration").unwrap();
        let metadata = serde_json::json!({"version":1,"audio":AudioSettings::default(),"track":{
            "name":"Legacy piano","plugin":sample().tracks[0].plugin}});
        let original = container(
            b"CLEFPRJ1",
            &serde_json::to_vec(&metadata).unwrap(),
            &[0, 255, 42],
        );
        let path = directory.0.join("Legacy.clef");
        std::fs::write(&path, &original).unwrap();
        let project = Project::read(&path).unwrap();
        assert_eq!(project.version, 2);
        assert_eq!(project.tracks[0].id, 1);
        assert_eq!(project.tracks[0].state, vec![0, 255, 42]);
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let copy = directory.0.join("Migrated.clef");
        project.save(&copy, false).unwrap();
        assert_eq!(Project::read(&copy).unwrap(), project);
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let empty = serde_json::json!({"version":1,"audio":AudioSettings::default(),"track":null});
        assert!(
            Project::decode(
                &mut container(b"CLEFPRJ1", &serde_json::to_vec(&empty).unwrap(), &[]).as_slice()
            )
            .unwrap()
            .tracks
            .is_empty()
        );
    }
    #[test]
    fn distinct_states_order_and_identity_survive_multitrack_roundtrip() {
        let mut project = sample();
        for id in 2..=8 {
            let mut track = project.tracks[0].clone();
            track.id = id;
            track.name = format!("Track {id}");
            track.state = vec![id as u8; id as usize];
            project.tracks.push(track);
        }
        project.next_id = 9;
        project.tracks.reverse();
        let mut bytes = Vec::new();
        project.write(&mut bytes).unwrap();
        assert_eq!(Project::decode(&mut bytes.as_slice()).unwrap(), project);
        project.tracks[1].id = project.tracks[0].id;
        assert!(project.validate().is_err());
    }
    #[test]
    fn state_table_and_duplicate_json_fields_are_rejected() {
        let project = sample();
        let mut value = serde_json::to_value(&project).unwrap();
        value["state_lengths"] = serde_json::json!([1]);
        let bad = container(b"CLEFPRJ2", &serde_json::to_vec(&value).unwrap(), &[1, 2]);
        assert!(Project::decode(&mut bad.as_slice()).is_err());
        let mut raw = serde_json::to_string(&value).unwrap();
        raw.insert_str(1, "\"version\":2,");
        assert!(
            Project::decode(&mut container(b"CLEFPRJ2", raw.as_bytes(), &[1]).as_slice()).is_err()
        );
        value["tracks"][0]["plugin"] = serde_json::Value::Null;
        assert!(
            Project::decode(
                &mut container(b"CLEFPRJ2", &serde_json::to_vec(&value).unwrap(), &[1]).as_slice()
            )
            .is_err()
        );
    }
    #[test]
    fn empty_and_maximum_track_sessions_round_trip_and_invalid_tables_are_rejected() {
        for count in [0, 8, MAX_TRACKS] {
            let project = Project {
                tracks: (1..=count as u64)
                    .map(|id| Track {
                        id,
                        name: format!("Track {id}"),
                        plugin: None,
                        mix: Default::default(),
                        state: vec![],
                    })
                    .collect(),
                next_id: count as u64 + 1,
                ..Default::default()
            };
            let mut bytes = Vec::new();
            project.write(&mut bytes).unwrap();
            assert_eq!(Project::decode(&mut bytes.as_slice()).unwrap(), project);
        }
        let mut project = sample();
        project.tracks.push(project.tracks[0].clone());
        assert!(project.validate().is_err());
        let mut value = serde_json::to_value(sample()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("state_lengths".into(), serde_json::json!([3]));
        let bytes = container(MAGIC, &serde_json::to_vec(&value).unwrap(), &[1, 2, 3, 4]);
        assert!(Project::decode(&mut bytes.as_slice()).is_err());
        value["state_lengths"] = serde_json::json!([4]);
        value["unknown"] = true.into();
        let bytes = container(MAGIC, &serde_json::to_vec(&value).unwrap(), &[1, 2, 3, 4]);
        assert!(Project::decode(&mut bytes.as_slice()).is_err());
    }
}
