use crate::protocol::{BLOCK, Frame, MAX_EVENTS, MAX_STATE, Midi, Result};
use clack_extensions::{
    audio_ports::{AudioPortInfoBuffer, PluginAudioPorts},
    gui::{GuiSize, HostGui, HostGuiImpl, PluginGui},
    latency::{HostLatency, HostLatencyImpl, PluginLatency},
    note_ports::{NoteDialects, NotePortInfoBuffer, PluginNotePorts},
    params::{
        HostParams, HostParamsImplMainThread, HostParamsImplShared, ParamClearFlags,
        ParamRescanFlags, PluginParams,
    },
    render::{PluginRender, RenderMode},
    state::PluginState,
    thread_check::{HostThreadCheck, HostThreadCheckImpl},
    timer::{HostTimer, HostTimerImpl, PluginTimer, TimerId},
};
use clack_host::{
    events::{
        EventFlags,
        event_types::{MidiEvent, TransportEvent, TransportFlags},
    },
    prelude::*,
    utils::{BeatTime, SecondsTime},
};
use std::{
    cell::{Cell, RefCell},
    ffi::CString,
    path::Path,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::ThreadId,
    time::{Duration, Instant},
};

pub struct Signals {
    pub restart: AtomicBool,
    callback: AtomicBool,
    flush: AtomicBool,
    pub resize: AtomicU64,
    pub closed: AtomicBool,
    pub show: AtomicBool,
    pub hide: AtomicBool,
    main_thread: ThreadId,
    audio_thread: OnceLock<ThreadId>,
}

pub struct Host;
pub struct Shared(Arc<Signals>);
pub struct Main<'a> {
    shared: &'a Shared,
    timers: RefCell<Vec<(TimerId, Duration, Instant)>>,
    next_timer: Cell<u32>,
}

impl HostHandlers for Host {
    type Shared<'a> = Shared;
    type MainThread<'a> = Main<'a>;
    type AudioProcessor<'a> = ();
    fn declare_extensions(builder: &mut HostExtensions<Self>, _: &Shared) {
        builder
            .register::<HostGui>()
            .register::<HostTimer>()
            .register::<HostLatency>()
            .register::<HostParams>()
            .register::<HostThreadCheck>();
    }
}
impl<'a> SharedHandler<'a> for Shared {
    fn request_restart(&self) {
        self.0.restart.store(true, Ordering::Release);
    }
    fn request_process(&self) {}
    fn request_callback(&self) {
        self.0.callback.store(true, Ordering::Release);
    }
}
impl<'a> MainThreadHandler<'a> for Main<'a> {}
impl HostLatencyImpl for Main<'_> {
    fn changed(&self) {
        self.shared.0.restart.store(true, Ordering::Release);
    }
}
impl HostThreadCheckImpl for Shared {
    fn is_main_thread(&self) -> bool {
        std::thread::current().id() == self.0.main_thread
    }
    fn is_audio_thread(&self) -> bool {
        self.0.audio_thread.get() == Some(&std::thread::current().id())
    }
}
impl HostParamsImplShared for Shared {
    fn request_flush(&self) {
        self.0.flush.store(true, Ordering::Release);
    }
}
impl HostParamsImplMainThread for Main<'_> {
    fn rescan(&self, _: ParamRescanFlags) {}
    fn clear(&self, _: ClapId, _: ParamClearFlags) {}
}
impl HostGuiImpl for Shared {
    fn resize_hints_changed(&self) {}
    fn request_resize(&self, size: GuiSize) -> std::result::Result<(), HostError> {
        if size.width == 0 || size.height == 0 || size.width > 8192 || size.height > 8192 {
            return Err(HostError::Message("invalid GUI size"));
        }
        self.0.resize.store(
            (u64::from(size.width) << 32) | u64::from(size.height),
            Ordering::Release,
        );
        Ok(())
    }
    fn request_show(&self) -> std::result::Result<(), HostError> {
        self.0.show.store(true, Ordering::Release);
        Ok(())
    }
    fn request_hide(&self) -> std::result::Result<(), HostError> {
        self.0.hide.store(true, Ordering::Release);
        Ok(())
    }
    fn closed(&self, _: bool) {
        self.0.closed.store(true, Ordering::Release);
    }
}
impl HostTimerImpl for Main<'_> {
    fn register_timer(&self, period_ms: u32) -> std::result::Result<TimerId, HostError> {
        let mut timers = self.timers.borrow_mut();
        if period_ms == 0 || timers.len() >= 64 || self.next_timer.get() == u32::MAX {
            return Err(HostError::Message("timer limit reached"));
        }
        let id = TimerId(self.next_timer.get());
        self.next_timer.set(id.0 + 1);
        let period = Duration::from_millis(u64::from(period_ms.max(1)));
        timers.push((id, period, Instant::now() + period));
        Ok(id)
    }
    fn unregister_timer(&self, id: TimerId) -> std::result::Result<(), HostError> {
        let mut timers = self.timers.borrow_mut();
        let index = timers
            .iter()
            .position(|(timer, _, _)| *timer == id)
            .ok_or(HostError::Message("unknown timer"))?;
        timers.swap_remove(index);
        Ok(())
    }
}

pub struct ClapMain {
    pub instance: PluginInstance<Host>,
    pub name: String,
    pub signals: Arc<Signals>,
    pub gui: Option<PluginGui>,
    pub gui_open: bool,
    sample_rate: u32,
    effect: bool,
    input_ports: Vec<usize>,
    output_ports: Vec<usize>,
    main_output: usize,
}

/// Most audio ports per direction an instrument may declare (T43).
const MAX_PORTS: usize = 128;
/// Most channels in one port.
const MAX_PORT_CHANNELS: usize = 32;

/// T43: picks the output port an instrument is heard on (the first flagged main, else port 0)
/// from each output's channel count and main flag, and checks the layout Clef can host: a mono
/// or stereo main output and at most `MAX_PORT_CHANNELS` channels in any port.
fn instrument_main_output(outputs: &[(usize, bool)], inputs: &[usize]) -> Result<usize> {
    let main = outputs.iter().position(|&(_, main)| main).unwrap_or(0);
    let channels = outputs
        .get(main)
        .ok_or("CLAP instrument has no audio output")?
        .0;
    if !(1..=2).contains(&channels) {
        return Err(format!(
            "CLAP main output has {channels} channels; Clef plays mono or stereo instruments only"
        )
        .into());
    }
    if outputs
        .iter()
        .map(|&(count, _)| count)
        .chain(inputs.iter().copied())
        .any(|count| count > MAX_PORT_CHANNELS)
    {
        return Err("CLAP instrument port has too many channels".into());
    }
    Ok(main)
}

impl ClapMain {
    pub fn load(path: &Path, selected: &str, sample_rate: u32, effect: bool) -> Result<Self> {
        // Native code is loaded only inside the disposable plugin worker.
        let entry = unsafe { PluginEntry::load(path) }.map_err(|e| e.to_string())?;
        let factory = entry
            .get_plugin_factory()
            .ok_or("CLAP entry has no plugin factory")?;
        let descriptor = factory
            .plugin_descriptors()
            .find(|d| {
                selected.is_empty()
                    || d.id()
                        .is_some_and(|id| id.to_bytes() == selected.as_bytes())
            })
            .ok_or("CLAP entry has no plugins")?;
        let id = CString::from(descriptor.id().ok_or("CLAP descriptor has no ID")?);
        let name = descriptor
            .name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let signals = Arc::new(Signals {
            restart: AtomicBool::new(false),
            callback: AtomicBool::new(false),
            flush: AtomicBool::new(false),
            resize: AtomicU64::new(0),
            closed: AtomicBool::new(false),
            show: AtomicBool::new(false),
            hide: AtomicBool::new(false),
            main_thread: std::thread::current().id(),
            audio_thread: OnceLock::new(),
        });
        let host_info = HostInfo::new(
            "Clef",
            "Clef",
            "https://localhost",
            env!("CARGO_PKG_VERSION"),
        )?;
        let shared_signals = Arc::clone(&signals);
        let mut instance = PluginInstance::<Host>::new(
            move |_| Shared(shared_signals),
            |shared| Main {
                shared,
                timers: RefCell::new(Vec::new()),
                next_timer: Cell::new(0),
            },
            &entry,
            &id,
            &host_info,
        )
        .map_err(|e| e.to_string())?;
        let handle = instance.plugin_handle();
        let ports = handle
            .get_extension::<PluginAudioPorts>()
            .ok_or("CLAP plugin has no audio-port extension")?;
        let mut buffer = AudioPortInfoBuffer::new();
        let output_count = ports.count(&handle, false) as usize;
        let input_count = ports.count(&handle, true) as usize;
        let mut input_ports = Vec::new();
        let mut output_ports = Vec::new();
        let main_output;
        if effect {
            let output = ports
                .get(&handle, 0, false, &mut buffer)
                .ok_or("CLAP plugin has no output")?;
            if output_count != 1 || output.channel_count != 2 || !(1..=32).contains(&input_count) {
                return Err("unsupported CLAP audio port layout".into());
            }
            output_ports.push(2);
            main_output = 0;
            for index in 0..input_count {
                let input = ports
                    .get(&handle, index as u32, true, &mut buffer)
                    .ok_or("CLAP effect has an unavailable input port")?;
                if index == 0 && input.channel_count != 2
                    || index != 0
                        && (input
                            .flags
                            .contains(clack_extensions::audio_ports::AudioPortFlags::IS_MAIN)
                            || !(1..=2).contains(&input.channel_count))
                {
                    return Err(
                        "CLAP effect requires a stereo main input and mono or stereo auxiliary inputs"
                            .into(),
                    );
                }
                input_ports.push(input.channel_count as usize);
            }
        } else {
            // T43: an instrument may have several outputs, of which Clef plays the main one
            // (flagged main, else port 0) in mono or stereo, and inputs (sidechain), which
            // receive silence.
            if output_count == 0 {
                return Err("CLAP instrument has no audio output".into());
            }
            if output_count > MAX_PORTS || input_count > MAX_PORTS {
                return Err(format!(
                    "CLAP instrument declares {output_count} output and {input_count} input \
                     ports; Clef hosts at most {MAX_PORTS} of each"
                )
                .into());
            }
            let mut outputs = Vec::new();
            for index in 0..output_count {
                let port = ports
                    .get(&handle, index as u32, false, &mut buffer)
                    .ok_or("CLAP instrument has an unavailable output port")?;
                outputs.push((
                    port.channel_count as usize,
                    port.flags
                        .contains(clack_extensions::audio_ports::AudioPortFlags::IS_MAIN),
                ));
            }
            for index in 0..input_count {
                let port = ports
                    .get(&handle, index as u32, true, &mut buffer)
                    .ok_or("CLAP instrument has an unavailable input port")?;
                input_ports.push(port.channel_count as usize);
            }
            main_output = instrument_main_output(&outputs, &input_ports)?;
            output_ports = outputs.into_iter().map(|(channels, _)| channels).collect();
        }
        if !effect {
            let notes = handle
                .get_extension::<PluginNotePorts>()
                .ok_or("CLAP instrument has no note input")?;
            let mut note_buffer = NotePortInfoBuffer::new();
            let note = notes
                .get(&handle, 0, true, &mut note_buffer)
                .ok_or("CLAP instrument has no note input")?;
            if !note.supported_dialects.contains(NoteDialects::MIDI) {
                return Err("T02 requires MIDI 1.0 on CLAP note port 0".into());
            }
        }
        let gui = handle.get_extension::<PluginGui>();
        Ok(Self {
            instance,
            name,
            signals,
            gui,
            gui_open: false,
            sample_rate,
            effect,
            input_ports,
            output_ports,
            main_output,
        })
    }

    pub fn activate(&mut self, offline: bool) -> Result<ClapAudio> {
        let handle = self.instance.plugin_handle();
        if let Some(render) = handle.get_extension::<PluginRender>() {
            if offline && render.has_realtime_requirement(&handle) {
                return Err("CLAP plugin requires realtime rendering".into());
            }
            render.set(
                &handle,
                if offline {
                    RenderMode::Offline
                } else {
                    RenderMode::Realtime
                },
            )?;
        }
        let processor = self
            .instance
            .activate(
                |_, _| (),
                PluginAudioConfiguration {
                    sample_rate: f64::from(self.sample_rate),
                    min_frames_count: 1,
                    max_frames_count: BLOCK as u32,
                },
            )
            .map_err(|e| e.to_string())?;
        self.signals.restart.store(false, Ordering::Release);
        Ok(ClapAudio {
            processor: processor.into(),
            events: EventBuffer::with_capacity(MAX_EVENTS),
            outputs: AudioPorts::with_capacity(
                self.output_ports.iter().sum(),
                self.output_ports.len(),
            ),
            inputs: AudioPorts::with_capacity(
                self.input_ports.iter().sum(),
                self.input_ports.len(),
            ),
            input_channels: self
                .input_ports
                .iter()
                .map(|&count| vec![[0.0; BLOCK]; count])
                .collect(),
            output_channels: self
                .output_ports
                .iter()
                .map(|&count| vec![[0.0; BLOCK]; count])
                .collect(),
            effect: self.effect,
            main_output: self.main_output,
            steady: 0,
            sample_rate: self.sample_rate,
            held: HeldNotes::new(),
            signals: Arc::clone(&self.signals),
        })
    }

    pub fn deactivate(&mut self, audio: ClapAudio) {
        self.instance.deactivate(audio.processor.into_stopped());
    }

    pub fn latency(&mut self) -> Option<u32> {
        // CLAP permits latency queries only during activation or while active.
        self.instance.is_active().then(|| {
            let handle = self.instance.plugin_handle();
            handle
                .get_extension::<PluginLatency>()
                .map_or(0, |latency| latency.get(&handle))
        })
    }

    pub fn save(&mut self) -> Result<Vec<u8>> {
        let handle = self.instance.plugin_handle();
        let state = handle
            .get_extension::<PluginState>()
            .ok_or("CLAP plugin does not support state")?;
        struct Limited(Vec<u8>);
        impl std::io::Write for Limited {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.0.len().saturating_add(bytes.len()) > MAX_STATE {
                    return Err(std::io::Error::other("state exceeds limit"));
                }
                self.0.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut bytes = Limited(Vec::new());
        state.save(&handle, &mut bytes)?;
        Ok(bytes.0)
    }

    pub fn restore(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_STATE {
            return Err("state exceeds limit".into());
        }
        let handle = self.instance.plugin_handle();
        handle
            .get_extension::<PluginState>()
            .ok_or("CLAP plugin does not support state")?
            .load(&handle, &mut &bytes[..])?;
        Ok(())
    }

    pub fn service(&mut self) {
        if !self.instance.is_active()
            && self.signals.flush.swap(false, Ordering::AcqRel)
            && let Some(mut handle) = self.instance.inactive_plugin_handle()
            && let Some(params) = handle.get_extension::<PluginParams>()
        {
            params.flush(
                &mut handle,
                &InputEvents::empty(),
                &mut OutputEvents::void(),
            );
        }
        if self.signals.callback.swap(false, Ordering::AcqRel) {
            self.instance.call_on_main_thread_callback();
        }
        let now = Instant::now();
        let due = self.instance.access_handler(|h| {
            let mut due = Vec::new();
            for (id, period, next) in h.timers.borrow_mut().iter_mut() {
                if now >= *next {
                    due.push(*id);
                    *next = now + *period;
                }
            }
            due
        });
        let handle = self.instance.plugin_handle();
        if let Some(timer) = handle.get_extension::<PluginTimer>() {
            for id in due {
                timer.on_timer(&handle, id);
            }
        }
    }
}

pub struct ClapAudio {
    pub processor: PluginAudioProcessor<Host>,
    events: EventBuffer,
    outputs: AudioPorts,
    inputs: AudioPorts,
    /// One buffer per channel of each input port; only an effect's port 0 carries the track.
    input_channels: Vec<Vec<[f32; BLOCK]>>,
    /// One buffer per channel of each output port; Clef plays `main_output`.
    output_channels: Vec<Vec<[f32; BLOCK]>>,
    effect: bool,
    main_output: usize,
    steady: u64,
    sample_rate: u32,
    held: HeldNotes,
    signals: Arc<Signals>,
}

impl ClapAudio {
    pub fn stop(&mut self) -> Result<()> {
        if !self.effect {
            let cleanup = crate::protocol::panic_events();
            self.process(0, &cleanup, BLOCK, &[])?;
        }
        self.processor.ensure_processing_stopped();
        Ok(())
    }
    pub fn process(
        &mut self,
        position: u64,
        events: &[Midi],
        frames: usize,
        input: &[Frame],
    ) -> Result<Vec<Frame>> {
        let effect = self.effect;
        if (effect && input.len() != frames) || (!effect && !input.is_empty()) {
            return Err("wrong CLAP input role or length".into());
        }
        let _ = self.signals.audio_thread.set(std::thread::current().id());
        self.events.clear();
        for event in events {
            self.held.track(event.bytes, |bytes| {
                self.events.push(&MidiEvent::new(event.offset, 0, bytes));
            })?;
            self.events.push(
                &MidiEvent::new(event.offset, 0, event.bytes).with_flags(EventFlags::IS_LIVE),
            );
        }
        for port in &mut self.output_channels {
            for channel in port {
                channel[..frames].fill(0.0);
            }
        }
        if effect {
            for (i, frame) in input.iter().enumerate() {
                self.input_channels[0][0][i] = frame[0];
                self.input_channels[0][1][i] = frame[1];
            }
        }
        let inputs =
            self.inputs
                .with_input_buffers(self.input_channels.iter_mut().enumerate().map(
                    |(index, channels)| AudioPortBuffer {
                        latency: 0,
                        channels: AudioPortBufferType::f32_input_only(channels.iter_mut().map(
                            move |channel| {
                                if effect && index == 0 {
                                    InputChannel::variable(&mut channel[..frames])
                                } else {
                                    channel[..frames].fill(0.0);
                                    InputChannel::constant(&mut channel[..frames])
                                }
                            },
                        )),
                    },
                ));
        let mut outputs =
            self.outputs
                .with_output_buffers(self.output_channels.iter_mut().map(|channels| {
                    AudioPortBuffer {
                        latency: 0,
                        channels: AudioPortBufferType::f32_output_only(
                            channels.iter_mut().map(|channel| &mut channel[..frames]),
                        ),
                    }
                }));
        let seconds = position as f64 / f64::from(self.sample_rate);
        let beats = seconds * 100.0 / 60.0;
        let transport = TransportEvent {
            header: Default::default(),
            flags: TransportFlags::HAS_TEMPO
                | TransportFlags::HAS_BEATS_TIMELINE
                | TransportFlags::HAS_SECONDS_TIMELINE
                | TransportFlags::HAS_TIME_SIGNATURE
                | TransportFlags::IS_PLAYING,
            song_pos_beats: BeatTime::from_float(beats),
            song_pos_seconds: SecondsTime::from_float(seconds),
            tempo: 100.0,
            tempo_inc: 0.0,
            loop_start_beats: BeatTime::from_float(0.0),
            loop_end_beats: BeatTime::from_float(0.0),
            loop_start_seconds: SecondsTime::from_float(0.0),
            loop_end_seconds: SecondsTime::from_float(0.0),
            bar_start: BeatTime::from_float((beats / 4.0).floor() * 4.0),
            bar_number: (beats / 4.0) as i32,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        self.processor
            .ensure_processing_started()
            .map_err(|e| e.to_string())?
            .process(
                &inputs,
                &mut outputs,
                &self.events.as_input(),
                &mut OutputEvents::void(),
                Some(self.steady),
                Some(&transport),
            )
            .map_err(|e| e.to_string())?;
        // Active process calls already exchange parameter changes in both directions.
        self.signals.flush.store(false, Ordering::Release);
        self.steady += frames as u64;
        // A mono main output plays on both sides.
        let main = &self.output_channels[self.main_output];
        let right = main.len() - 1;
        Ok((0..frames).map(|i| [main[0][i], main[right][i]]).collect())
    }
}

struct HeldNotes {
    notes: Box<[[u16; 128]; 16]>,
    total: usize,
}
impl HeldNotes {
    fn new() -> Self {
        Self {
            notes: Box::new([[0; 128]; 16]),
            total: 0,
        }
    }
    fn track(&mut self, bytes: [u8; 3], mut note_off: impl FnMut([u8; 3])) -> Result<()> {
        let [status, key, value] = bytes;
        let channel = usize::from(status & 15);
        match status & 0xf0 {
            0x90 if value > 0 => {
                if self.total == MAX_EVENTS {
                    return Err("CLAP held-note tracking capacity exceeded".into());
                }
                self.notes[channel][usize::from(key)] += 1;
                self.total += 1;
            }
            0x90 | 0x80 => {
                let count = &mut self.notes[channel][usize::from(key)];
                if *count > 0 {
                    *count -= 1;
                    self.total -= 1;
                }
            }
            0xb0 if key == 120 || key == 123 => {
                for (note, count) in self.notes[channel].iter_mut().enumerate() {
                    while *count > 0 {
                        note_off([0x80 | channel as u8, note as u8, 0]);
                        *count -= 1;
                        self.total -= 1;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_note_ons_are_counted_and_bounded_before_cleanup() {
        let mut held = HeldNotes::new();
        for _ in 0..MAX_EVENTS {
            held.track([0x93, 60, 100], |_| {}).unwrap();
        }
        assert!(held.track([0x93, 60, 100], |_| {}).is_err());
        let mut offs = Vec::new();
        held.track([0xb3, 123, 0], |bytes| offs.push(bytes))
            .unwrap();
        assert_eq!(offs, vec![[0x83, 60, 0]; MAX_EVENTS]);
        assert_eq!(held.total, 0);
    }

    #[test]
    fn panic_explicitly_releases_held_notes_on_their_original_channels() {
        let mut held = HeldNotes::new();
        let mut offs = Vec::new();
        for event in [
            [0x92, 60, 100],
            [0x9f, 72, 80],
            [0x92, 61, 90],
            [0x92, 61, 0],
        ] {
            held.track(event, |bytes| offs.push(bytes)).unwrap();
        }
        for event in crate::protocol::panic_events() {
            held.track(event.bytes, |bytes| offs.push(bytes)).unwrap();
        }
        assert_eq!(offs, vec![[0x82, 60, 0], [0x8f, 72, 0]]);
        assert_eq!(held.total, 0);
        held.track([0xbf, 123, 0], |bytes| offs.push(bytes))
            .unwrap();
        assert_eq!(offs.len(), 2);
    }

    #[test]
    fn t43_instruments_play_their_main_output_in_mono_or_stereo_only() {
        // Several outputs: the port flagged main wins, else port 0.
        assert_eq!(
            instrument_main_output(&[(2, false), (2, true), (2, false)], &[2]).unwrap(),
            1
        );
        assert_eq!(
            instrument_main_output(&[(2, false), (2, false)], &[]).unwrap(),
            0
        );
        assert_eq!(instrument_main_output(&[(1, true)], &[]).unwrap(), 0);
        // Surround main outputs and oversized ports are refused with a reason.
        let surround = instrument_main_output(&[(6, true), (2, false)], &[])
            .unwrap_err()
            .to_string();
        assert!(surround.contains("6 channels"), "{surround}");
        assert!(instrument_main_output(&[(2, true)], &[33]).is_err());
    }
}
