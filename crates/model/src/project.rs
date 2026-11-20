use crate::{DEFAULT_BLOCK_SIZE as BLOCK, DEFAULT_SAMPLE_RATE as RATE, MAX_STATE, Plugin, Result};
use crate::{
    midi::{Arrangement, Mode, Snap},
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
pub const MAX_SEND_EDGES: usize = 512;
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
pub const TAKE_LANE_DEFAULT_HEIGHT: u16 = 40;
pub const TAKE_LANE_MIN_HEIGHT: u16 = 24;
pub const TAKE_LANE_MAX_HEIGHT: u16 = 320;
const TAKE_LANE_HEIGHT_STEP: u16 = 4;
const MAX_TAKE_LANES: usize = 4096;
const MAX_TAKE_NAME: usize = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackMode {
    #[default]
    Global,
    Merge,
    Replace,
    NewTake,
}

impl TrackMode {
    pub const ALL: [Self; 4] = [Self::Global, Self::Merge, Self::Replace, Self::NewTake];

    pub fn label(self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Merge => "Merge",
            Self::Replace => "Replace",
            Self::NewTake => "New take",
        }
    }

    pub fn effective(self, global: Mode) -> Mode {
        match self {
            Self::Global => global,
            Self::Merge => Mode::Merge,
            Self::Replace => Mode::Replace,
            Self::NewTake => Mode::NewTake,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakeLane {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        default = "default_take_lane_height",
        skip_serializing_if = "is_default_take_lane_height"
    )]
    pub height: u16,
}

impl Default for TakeLane {
    fn default() -> Self {
        Self {
            name: None,
            height: TAKE_LANE_DEFAULT_HEIGHT,
        }
    }
}

fn default_take_lane_height() -> u16 {
    TAKE_LANE_DEFAULT_HEIGHT
}

fn is_default_take_lane_height(height: &u16) -> bool {
    *height == TAKE_LANE_DEFAULT_HEIGHT
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakeLanes {
    #[serde(default)]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lanes: Vec<TakeLane>,
}

impl TakeLanes {
    pub fn lane(&self, index: usize) -> Option<&TakeLane> {
        self.lanes.get(index)
    }

    pub fn name(&self, index: usize) -> String {
        self.lanes
            .get(index)
            .and_then(|lane| lane.name.clone())
            .unwrap_or_else(|| format!("Take {}", index + 1))
    }

    pub fn height(&self, index: usize) -> u16 {
        self.lanes
            .get(index)
            .map_or(TAKE_LANE_DEFAULT_HEIGHT, |lane| lane.height)
    }

    pub fn set_name(&mut self, index: usize, name: Option<String>) {
        let lane = self.lane_mut(index);
        lane.name = name;
    }

    pub fn set_height(&mut self, index: usize, height: u16) {
        self.lane_mut(index).height = height;
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.lanes.len() {
            self.lanes.remove(index);
        }
    }

    fn lane_mut(&mut self, index: usize) -> &mut TakeLane {
        while self.lanes.len() <= index {
            self.lanes.push(TakeLane::default());
        }
        &mut self.lanes[index]
    }

    pub fn validate(&self) -> Result<()> {
        if self.lanes.len() > MAX_TAKE_LANES {
            return Err("invalid take lane count".into());
        }
        for lane in &self.lanes {
            let height = lane.height;
            if !(TAKE_LANE_MIN_HEIGHT..=TAKE_LANE_MAX_HEIGHT).contains(&height)
                || !(height - TAKE_LANE_MIN_HEIGHT).is_multiple_of(TAKE_LANE_HEIGHT_STEP)
            {
                return Err("invalid take lane height".into());
            }
            if let Some(name) = &lane.name
                && (name.trim().is_empty()
                    || name.chars().count() > MAX_TAKE_NAME
                    || name.contains('\0'))
            {
                return Err("invalid take lane name".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    #[default]
    Instrument,
    Bus,
}

impl TrackKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Instrument => "Instrument",
            Self::Bus => "Bus",
        }
    }

    pub fn is_bus(self) -> bool {
        self == Self::Bus
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Send {
    pub id: u64,
    pub target: u64,
    #[serde(default = "default_send_level")]
    pub level: i16,
    #[serde(default)]
    pub pre: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_send_level() -> i16 {
    crate::mixer::Control::Fader.bounds().0
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Insert {
    pub id: u64,
    pub plugin: Plugin,
    #[serde(default)]
    pub bypassed: bool,
    #[serde(skip)]
    pub state: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub id: u64,
    pub name: String,
    pub plugin: Option<Plugin>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub instrument_bypassed: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub instrument_offline: bool,
    #[serde(default)]
    pub record_mode: TrackMode,
    #[serde(default)]
    pub take_lanes: TakeLanes,
    #[serde(default)]
    pub kind: TrackKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sends: Vec<Send>,
    #[serde(default, skip_serializing_if = "crate::mixer::Channel::is_default")]
    pub mix: crate::mixer::Channel,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inserts: Vec<Insert>,
    /// Opaque instrument state, shared between the session, history and autosave snapshots.
    #[serde(skip)]
    pub state: Arc<[u8]>,
}

impl Track {
    pub fn is_bus(&self) -> bool {
        self.kind.is_bus()
    }

    pub fn is_instrument(&self) -> bool {
        !self.is_bus()
    }

    pub fn output_bus(&self) -> Option<u64> {
        self.output
    }
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
    pub record_mode: Mode,
    #[serde(default)]
    pub midi: Arc<Arrangement>,
    #[serde(default)]
    pub arrangement_view: ArrangementView,
    #[serde(default)]
    pub master: crate::mixer::Level,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub master_inserts: Vec<Insert>,
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
    #[serde(default, skip_serializing_if = "is_merge")]
    record_mode: Mode,
    #[serde(default, skip_serializing_if = "Arrangement::is_default")]
    midi: Arc<Arrangement>,
    #[serde(default, skip_serializing_if = "default_arrangement_view")]
    arrangement_view: ArrangementView,
    #[serde(default, skip_serializing_if = "crate::mixer::Level::is_default")]
    master: crate::mixer::Level,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    master_inserts: Vec<Insert>,
    state_lengths: Vec<usize>,
}

fn default_arrangement_view(view: &ArrangementView) -> bool {
    *view == ArrangementView::default()
}

fn is_merge(mode: &Mode) -> bool {
    *mode == Mode::Merge
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
            version: 4,
            tracks: Vec::new(),
            next_id: 1,
            audio: AudioSettings::default(),
            timeline: Timeline::default(),
            record_mode: Mode::Merge,
            midi: Arc::default(),
            arrangement_view: ArrangementView::default(),
            master: Default::default(),
            master_inserts: Vec::new(),
        }
    }
}

impl Project {
    pub fn inserts(&self, target: crate::mixer::Target) -> Option<&[Insert]> {
        match target {
            crate::mixer::Target::Master => Some(&self.master_inserts),
            crate::mixer::Target::Track(id) | crate::mixer::Target::Bus(id) => self
                .tracks
                .iter()
                .find(|t| t.id == id)
                .map(|track| track.inserts.as_slice()),
        }
    }

    pub fn inserts_mut(&mut self, target: crate::mixer::Target) -> Option<&mut Vec<Insert>> {
        match target {
            crate::mixer::Target::Master => Some(&mut self.master_inserts),
            crate::mixer::Target::Track(id) | crate::mixer::Target::Bus(id) => self
                .tracks
                .iter_mut()
                .find(|t| t.id == id)
                .map(|track| &mut track.inserts),
        }
    }

    pub fn bus_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.tracks.iter().filter(|t| t.is_bus()).map(|t| t.id)
    }

    pub fn instrument_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.tracks
            .iter()
            .filter(|t| t.is_instrument())
            .map(|t| t.id)
    }

    pub fn bus(&self, id: u64) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id && t.is_bus())
    }

    pub fn has_sources(&self, bus: u64) -> usize {
        self.tracks
            .iter()
            .filter(|t| t.is_instrument())
            .filter(|t| t.output == Some(bus) || t.sends.iter().any(|send| send.target == bus))
            .count()
    }

    fn all_inserts(&self) -> impl Iterator<Item = &Insert> {
        self.tracks
            .iter()
            .flat_map(|track| &track.inserts)
            .chain(&self.master_inserts)
    }

    pub fn mix(&self, target: crate::mixer::Target) -> Result<crate::mixer::Channel> {
        match target {
            crate::mixer::Target::Master => Ok(crate::mixer::Channel {
                level: self.master,
                ..Default::default()
            }),
            crate::mixer::Target::Track(id) | crate::mixer::Target::Bus(id) => self
                .tracks
                .iter()
                .find(|track| track.id == id)
                .map(|track| track.mix)
                .ok_or_else(|| "Mixer channel is unavailable.".into()),
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
            crate::mixer::Target::Track(id) | crate::mixer::Target::Bus(id) => self
                .tracks
                .iter_mut()
                .find(|track| track.id == id)
                .ok_or("Mixer channel is unavailable.")?
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
        let valid_text = |text: &str| text.len() <= 4096 && !text.contains('\0');
        if self.version != 4 {
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
        let buses: std::collections::HashSet<u64> = self.bus_ids().collect();
        self.midi.validate(self.instrument_ids())?;
        let mut ids = std::collections::HashSet::new();
        let mut total = 0usize;
        let mut send_edges = 0usize;
        let valid_plugin = |plugin: &Plugin| {
            valid_text(&plugin.name)
                && valid_text(&plugin.id)
                && !plugin.id.is_empty()
                && plugin.path.is_absolute()
                && plugin.path.as_os_str().len() <= 16384
                && matches!(
                    (
                        plugin.format.as_str(),
                        plugin.path.extension().and_then(|s| s.to_str())
                    ),
                    ("VST3", Some("vst3")) | ("CLAP", Some("clap"))
                )
        };
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
            if track.is_bus() {
                if track.plugin.is_some()
                    || !track.state.is_empty()
                    || track.take_lanes != TakeLanes::default()
                    || track.instrument_bypassed
                    || track.instrument_offline
                    || track.output.is_some()
                    || !track.sends.is_empty()
                {
                    return Err("invalid bus track state".into());
                }
            } else {
                if let Some(output) = track.output
                    && !buses.contains(&output)
                {
                    return Err("invalid track output destination".into());
                }
                track.take_lanes.validate()?;
                if let Some(plugin) = &track.plugin {
                    if !valid_plugin(plugin) {
                        return Err("invalid plugin identity".into());
                    }
                } else if !track.state.is_empty() {
                    return Err("state has no associated plugin".into());
                }
                let mut targets = std::collections::HashSet::new();
                for send in &track.sends {
                    send_edges = send_edges.checked_add(1).ok_or("send edge overflow")?;
                    if send_edges > MAX_SEND_EDGES {
                        return Err("project send budget exceeded".into());
                    }
                    if send.id == 0 || send.id >= self.next_id || !ids.insert(send.id) {
                        return Err("invalid send identity".into());
                    }
                    if !buses.contains(&send.target) || send.target == track.id {
                        return Err("invalid send destination".into());
                    }
                    if !targets.insert(send.target) {
                        return Err("duplicate send destination".into());
                    }
                    if !crate::mixer::Control::Fader.accepts(send.level) {
                        return Err("invalid send level".into());
                    }
                }
            }
        }
        for insert in self.all_inserts() {
            if insert.id == 0
                || insert.id >= self.next_id
                || !ids.insert(insert.id)
                || !valid_plugin(&insert.plugin)
                || insert.state.len() > MAX_STATE
            {
                return Err("invalid insert identity or state size".into());
            }
            total = total
                .checked_add(insert.state.len())
                .ok_or("state size overflow")?;
            if total > MAX_TOTAL_STATE {
                return Err("combined state exceeds limit".into());
            }
        }
        Ok(())
    }
    fn state_lengths(&self) -> Vec<usize> {
        self.tracks
            .iter()
            .map(|t| t.state.len())
            .chain(self.all_inserts().map(|insert| insert.state.len()))
            .collect()
    }

    fn encode_metadata(&self, state_lengths: Vec<usize>) -> Result<Vec<u8>> {
        let metadata = serde_json::to_vec(&Metadata {
            version: self.version,
            tracks: self
                .tracks
                .iter()
                .map(|t| Track {
                    id: t.id,
                    name: t.name.clone(),
                    plugin: t.plugin.clone(),
                    instrument_bypassed: t.instrument_bypassed,
                    instrument_offline: t.instrument_offline,
                    record_mode: t.record_mode,
                    mix: t.mix,
                    take_lanes: t.take_lanes.clone(),
                    kind: t.kind,
                    output: t.output,
                    sends: t.sends.clone(),
                    inserts: t.inserts.clone(),
                    state: Arc::default(),
                })
                .collect(),
            next_id: self.next_id,
            audio: self.audio.clone(),
            timeline: self.timeline.clone(),
            record_mode: self.record_mode,
            midi: self.midi.clone(),
            arrangement_view: self.arrangement_view,
            master: self.master,
            master_inserts: self.master_inserts.clone(),
            state_lengths,
        })?;
        if metadata.len() > MAX_METADATA {
            return Err("project metadata exceeds limit".into());
        }
        Ok(metadata)
    }

    pub fn validate_storage(&self) -> Result<()> {
        self.validate()?;
        self.encode_metadata(self.state_lengths())?;
        Ok(())
    }

    fn write(&self, writer: &mut impl Write) -> Result<()> {
        self.validate()?;
        let lengths = self.state_lengths();
        let total: usize = lengths.iter().sum();
        let metadata = self.encode_metadata(lengths)?;
        writer.write_all(MAGIC)?;
        writer.write_all(&(metadata.len() as u32).to_le_bytes())?;
        writer.write_all(&(total as u32).to_le_bytes())?;
        writer.write_all(&metadata)?;
        for track in &self.tracks {
            writer.write_all(&track.state)?;
        }
        for insert in self.all_inserts() {
            writer.write_all(&insert.state)?;
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
                    instrument_bypassed: false,
                    instrument_offline: false,
                    record_mode: TrackMode::Global,
                    mix: Default::default(),
                    take_lanes: TakeLanes::default(),
                    kind: TrackKind::Instrument,
                    output: None,
                    sends: Vec::new(),
                    inserts: Vec::new(),
                    state: Arc::default(),
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
                    version: 4,
                    next_id: tracks.len() as u64 + 1,
                    tracks,
                    audio: legacy.audio,
                    timeline: Timeline::default(),
                    record_mode: Mode::Merge,
                    midi: Arc::default(),
                    arrangement_view: ArrangementView::default(),
                    master: Default::default(),
                    master_inserts: Vec::new(),
                },
                lengths,
            )
        } else {
            let metadata: Metadata = serde_json::from_slice(&metadata)?;
            if !(2..=4).contains(&metadata.version) {
                return Err("unsupported Clef project version".into());
            }
            let mut midi = metadata.midi;
            let record_mode = if metadata.version == 2 {
                let mode = midi.mode;
                Arc::make_mut(&mut midi).mode = Mode::Merge;
                mode
            } else {
                metadata.record_mode
            };
            (
                Self {
                    version: 4,
                    tracks: metadata.tracks,
                    next_id: metadata.next_id,
                    audio: metadata.audio,
                    timeline: metadata.timeline,
                    record_mode,
                    midi,
                    arrangement_view: metadata.arrangement_view,
                    master: metadata.master,
                    master_inserts: metadata.master_inserts,
                },
                metadata.state_lengths,
            )
        };
        project.validate()?;
        if lengths.len() != project.tracks.len() + project.all_inserts().count()
            || lengths.iter().any(|n| *n > MAX_STATE)
            || lengths
                .iter()
                .try_fold(0usize, |sum, n| sum.checked_add(*n))
                != Some(total)
        {
            return Err("invalid project state lengths".into());
        }
        let mut lengths = lengths.into_iter();
        for (track, length) in project.tracks.iter_mut().zip(&mut lengths) {
            if length != 0 && track.plugin.is_none() {
                return Err("orphan plugin state".into());
            }
            let mut state = vec![0; length];
            reader.read_exact(&mut state)?;
            track.state = state.into();
        }
        for (insert, length) in project
            .tracks
            .iter_mut()
            .flat_map(|track| &mut track.inserts)
            .chain(&mut project.master_inserts)
            .zip(lengths)
        {
            let mut state = vec![0; length];
            reader.read_exact(&mut state)?;
            insert.state = state.into();
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

    #[test]
    fn instrument_flags_round_trip_and_older_projects_default_online() {
        let mut project = sample();
        project.tracks[0].instrument_bypassed = true;
        project.tracks[0].instrument_offline = true;
        let mut bytes = Vec::new();
        project.write(&mut bytes).unwrap();
        let decoded = Project::decode(&mut bytes.as_slice()).unwrap();
        assert!(decoded.tracks[0].instrument_bypassed);
        assert!(decoded.tracks[0].instrument_offline);

        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&bytes[16..16 + length]).unwrap();
        for track in metadata["tracks"].as_array_mut().unwrap() {
            let track = track.as_object_mut().unwrap();
            track.remove("instrument_bypassed");
            track.remove("instrument_offline");
        }
        let old = container(
            MAGIC,
            &serde_json::to_vec(&metadata).unwrap(),
            &bytes[16 + length..],
        );
        let online = Project::decode(&mut old.as_slice()).unwrap();
        assert!(!online.tracks[0].instrument_bypassed);
        assert!(!online.tracks[0].instrument_offline);
    }

    #[test]
    fn insert_order_and_opaque_states_round_trip_after_track_states() {
        let mut project = sample();
        let mut bytes = Vec::new();
        project.write(&mut bytes).unwrap();
        let older = Project::decode(&mut bytes.as_slice()).unwrap();
        assert!(older.tracks[0].inserts.is_empty() && older.master_inserts.is_empty());

        let plugin = project.tracks[0].plugin.clone().unwrap();
        let insert = |id: u64| Insert {
            id,
            plugin: plugin.clone(),
            bypassed: id.is_multiple_of(2),
            state: Arc::from(id.to_le_bytes()),
        };
        project.tracks[0].inserts = (2..11).rev().map(insert).collect();
        project.master_inserts = vec![insert(11), insert(12)];
        project.next_id = 13;
        bytes.clear();
        project.write(&mut bytes).unwrap();
        assert_eq!(Project::decode(&mut bytes.as_slice()).unwrap(), project);
        project.master_inserts[0].id = 2;
        assert!(project.validate().is_err());
    }
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
                instrument_bypassed: false,
                instrument_offline: false,
                record_mode: TrackMode::Global,
                mix: Default::default(),
                inserts: Vec::new(),
                take_lanes: TakeLanes::default(),
                kind: TrackKind::Instrument,
                output: None,
                sends: Vec::new(),
                state: Arc::from([0, 255, 1, 2]),
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
    fn midi_modes_takes_and_sections_round_trip_and_migrate_v2() {
        use crate::midi::{Event, Message, Mode};
        let mut project = sample();
        project.record_mode = Mode::NewTake;
        let track = project.tracks[0].id;
        let midi = Arc::make_mut(&mut project.midi);
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
        let decoded = Project::decode(&mut bytes.as_slice()).unwrap();
        assert_eq!(decoded, project);
        assert_eq!(decoded.record_mode, Mode::NewTake);
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&bytes[16..16 + length]).unwrap();
        let object = metadata.as_object_mut().unwrap();
        object.insert("version".into(), serde_json::json!(2));
        object.remove("record_mode");
        object["midi"]
            .as_object_mut()
            .unwrap()
            .insert("mode".into(), serde_json::json!("NewTake"));
        let old = container(
            MAGIC,
            &serde_json::to_vec(&metadata).unwrap(),
            &bytes[16 + length..],
        );
        let migrated = Project::decode(&mut old.as_slice()).unwrap();
        assert_eq!(migrated.version, 4);
        assert_eq!(migrated.record_mode, Mode::NewTake);
        assert_eq!(migrated.midi.mode, Mode::Merge);
        assert_eq!(migrated.midi.clips(track), project.midi.clips(track));
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
        project.version = 5;
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
        changed.version = 5;
        assert!(changed.save(&path, true).is_err());
        assert_eq!(Project::read(&path).unwrap(), project);
        changed.version = 4;
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
        assert_eq!(project.version, 4);
        assert_eq!(project.tracks[0].id, 1);
        assert_eq!(*project.tracks[0].state, [0, 255, 42]);
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
            track.state = vec![id as u8; id as usize].into();
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
                        instrument_bypassed: false,
                        instrument_offline: false,
                        record_mode: TrackMode::Global,
                        mix: Default::default(),
                        inserts: Vec::new(),
                        take_lanes: TakeLanes::default(),
                        kind: TrackKind::Instrument,
                        output: None,
                        sends: Vec::new(),
                        state: Default::default(),
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
    #[test]
    fn record_modes_and_take_lanes_round_trip_and_validate() {
        let mut project = sample();
        project.record_mode = Mode::Replace;
        project.tracks[0].record_mode = TrackMode::NewTake;
        project.tracks[0].take_lanes = TakeLanes {
            visible: true,
            lanes: vec![
                TakeLane {
                    name: Some("Close".into()),
                    height: 64,
                },
                TakeLane::default(),
            ],
        };
        let mut bytes = Vec::new();
        project.write(&mut bytes).unwrap();
        assert_eq!(Project::decode(&mut bytes.as_slice()).unwrap(), project);
        project.tracks[0].take_lanes.lanes[0].height = 23;
        assert!(project.validate().is_err());
        project.tracks[0].take_lanes.lanes[0].height = 26;
        assert!(project.validate().is_err());
        project.tracks[0].take_lanes.lanes[0].height = 48;
        assert!(project.validate().is_ok());
        project.tracks[0].take_lanes.lanes[0].name = Some("   ".into());
        assert!(project.validate().is_err());
    }

    #[test]
    fn v2_without_take_fields_migrates_to_global_and_hidden_lanes() {
        let project = sample();
        let mut bytes = Vec::new();
        project.write(&mut bytes).unwrap();
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&bytes[16..16 + length]).unwrap();
        let object = metadata.as_object_mut().unwrap();
        object.insert("version".into(), serde_json::json!(2));
        object.remove("record_mode");
        for track in object["tracks"].as_array_mut().unwrap() {
            let track = track.as_object_mut().unwrap();
            track.remove("record_mode");
            track.remove("take_lanes");
        }
        let old = container(
            MAGIC,
            &serde_json::to_vec(&metadata).unwrap(),
            &bytes[16 + length..],
        );
        let migrated = Project::decode(&mut old.as_slice()).unwrap();
        assert_eq!(migrated.version, 4);
        assert_eq!(migrated.record_mode, Mode::Merge);
        assert_eq!(migrated.tracks[0].record_mode, TrackMode::Global);
        assert!(!migrated.tracks[0].take_lanes.visible);
        assert!(migrated.tracks[0].take_lanes.lanes.is_empty());
        assert!(migrated.tracks[0].inserts.is_empty());
        assert!(migrated.master_inserts.is_empty());
    }
    #[test]
    fn bus_output_and_sends_round_trip_and_reject_invalid_routing() {
        let mut project = sample();
        project.tracks.push(Track {
            id: 2,
            name: "Reverb".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: TrackMode::Global,
            mix: Default::default(),
            inserts: Vec::new(),
            take_lanes: TakeLanes::default(),
            kind: TrackKind::Bus,
            output: None,
            sends: Vec::new(),
            state: Default::default(),
        });
        project.tracks[0].output = Some(2);
        project.tracks[0].sends.push(Send {
            id: 3,
            target: 2,
            level: -60,
            pre: false,
            enabled: true,
        });
        project.next_id = 4;
        project.validate().unwrap();
        let mut bytes = Vec::new();
        project.write(&mut bytes).unwrap();
        assert_eq!(Project::decode(&mut bytes.as_slice()).unwrap(), project);

        let mut duplicate = project.clone();
        duplicate.tracks[0].sends.push(Send {
            id: 4,
            target: 2,
            level: 0,
            pre: true,
            enabled: true,
        });
        duplicate.next_id = 5;
        assert!(duplicate.validate().is_err());

        let mut wrong_output = project.clone();
        wrong_output.tracks[0].output = Some(1);
        assert!(wrong_output.validate().is_err());

        let mut bus_plugin = project.clone();
        bus_plugin.tracks[1].plugin = sample().tracks[0].plugin.clone();
        assert!(bus_plugin.validate().is_err());

        let mut bus_send = project.clone();
        bus_send.tracks[1].sends.push(Send {
            id: 4,
            target: 2,
            level: 0,
            pre: false,
            enabled: true,
        });
        bus_send.next_id = 5;
        assert!(bus_send.validate().is_err());
    }

    #[test]
    fn v3_metadata_migrates_to_v4_defaults() {
        let mut bytes = Vec::new();
        sample().write(&mut bytes).unwrap();
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&bytes[16..16 + length]).unwrap();
        metadata["version"] = serde_json::json!(3);
        for track in metadata["tracks"].as_array_mut().unwrap() {
            let track = track.as_object_mut().unwrap();
            track.remove("kind");
            track.remove("output");
            track.remove("sends");
        }
        let old = container(
            MAGIC,
            &serde_json::to_vec(&metadata).unwrap(),
            &bytes[16 + length..],
        );
        let migrated = Project::decode(&mut old.as_slice()).unwrap();
        assert_eq!(migrated.version, 4);
        assert_eq!(migrated.tracks[0].kind, TrackKind::Instrument);
        assert_eq!(migrated.tracks[0].output, None);
        assert!(migrated.tracks[0].sends.is_empty());
    }

    #[test]
    fn shared_instrument_state_keeps_the_container_byte_identical() {
        let mut project = sample();
        let plugin = project.tracks[0].plugin.clone().unwrap();
        project.tracks[0].inserts = vec![Insert {
            id: 2,
            plugin,
            bypassed: false,
            state: Arc::from([7, 8, 9]),
        }];
        project.next_id = 3;
        let mut bytes = Vec::new();
        project.write(&mut bytes).unwrap();
        // The pre-Arc container: header, metadata, then raw track states and insert states.
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let expected = container(MAGIC, &bytes[16..16 + length], &[0, 255, 1, 2, 7, 8, 9]);
        assert_eq!(bytes, expected);
        let decoded = Project::decode(&mut bytes.as_slice()).unwrap();
        assert_eq!(*decoded.tracks[0].state, [0, 255, 1, 2]);
        let snapshot = decoded.clone();
        assert!(Arc::ptr_eq(
            &snapshot.tracks[0].state,
            &decoded.tracks[0].state
        ));
        let mut again = Vec::new();
        decoded.write(&mut again).unwrap();
        assert_eq!(again, bytes);

        // Projects written by earlier builds re-encode to the same bytes when present locally.
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation-output");
        for entry in std::fs::read_dir(fixtures).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "clef") {
                continue;
            }
            let original = std::fs::read(&path).unwrap();
            let Ok(project) = Project::decode(&mut original.as_slice()) else {
                continue;
            };
            let metadata = &original
                [16..16 + u32::from_le_bytes(original[8..12].try_into().unwrap()) as usize];
            let state = &original[16 + metadata.len()..];
            let mut written = Vec::new();
            project.write(&mut written).unwrap();
            let length = u32::from_le_bytes(written[8..12].try_into().unwrap()) as usize;
            assert_eq!(&written[16 + length..], state, "{}", path.display());
        }
    }
}
