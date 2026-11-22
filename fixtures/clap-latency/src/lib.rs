//! Deterministic instrument or stereo effect with a real 0 -> 257 sample latency restart.
use clack_extensions::{audio_ports::*, latency::*, note_ports::*, render::*, state::*};
use clack_plugin::{
    events::spaces::CoreEventSpace,
    prelude::*,
    stream::{InputStream, OutputStream},
};
use std::{
    io::{Read, Write},
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
    thread::{self, ThreadId},
};

const EFFECT: bool = cfg!(feature = "effect-fixture");
/// T43: four output ports (0 is main) and one sidechain input port.
const MULTI: bool = cfg!(feature = "multi-bus-fixture");
/// T43: the multi-bus variant's main output is mono.
const MONO_MAIN: bool = cfg!(feature = "mono-main-fixture");
const OUTPUTS: u32 = if MULTI { 4 } else { 1 };
/// What the auxiliary outputs carry: a host that plays them is heard at this level.
const AUX_LEVEL: f32 = 0.9;

pub struct Fixture;
pub struct Shared<'a> {
    host: HostSharedHandle<'a>,
    main: ThreadId,
    requested: AtomicU32,
    applied: AtomicU32,
    notified: AtomicBool,
    offline: AtomicBool,
    active: AtomicBool,
}
pub struct Main<'a> {
    shared: &'a Shared<'a>,
    host: HostMainThreadHandle<'a>,
}
pub struct Audio<'a> {
    shared: &'a Shared<'a>,
    delay: [[f32; 2]; 257],
    cursor: usize,
    elapsed: u64,
    expression: [u8; 16],
    latency: usize,
}
impl Plugin for Fixture {
    type Shared<'a> = Shared<'a>;
    type MainThread<'a> = Main<'a>;
    type AudioProcessor<'a> = Audio<'a>;
    fn declare_extensions(builder: &mut PluginExtensions<Self>, _: Option<&Shared>) {
        builder
            .register::<PluginAudioPorts>()
            .register::<PluginNotePorts>()
            .register::<PluginLatency>()
            .register::<PluginRender>()
            .register::<PluginState>();
    }
}
impl DefaultPluginFactory for Fixture {
    fn get_descriptor() -> PluginDescriptor {
        use clack_plugin::plugin::features::*;
        if EFFECT {
            return PluginDescriptor::new("dev.clef.insert-fixture", "Clef CLAP Insert Fixture")
                .with_features([AUDIO_EFFECT, STEREO]);
        }
        if MONO_MAIN {
            return PluginDescriptor::new(
                "dev.clef.mono-main-fixture",
                "Clef CLAP Mono Main Fixture",
            )
            .with_features([INSTRUMENT, MONO]);
        }
        if MULTI {
            return PluginDescriptor::new(
                "dev.clef.multi-bus-fixture",
                "Clef CLAP Multi-Bus Fixture",
            )
            .with_features([INSTRUMENT, STEREO]);
        }
        PluginDescriptor::new("dev.clef.latency-fixture", "Clef CLAP Latency Fixture")
            .with_features([INSTRUMENT, STEREO])
    }
    fn new_shared(host: HostSharedHandle<'_>) -> Result<Shared<'_>, PluginError> {
        Ok(Shared {
            host,
            main: thread::current().id(),
            requested: AtomicU32::new(0),
            applied: AtomicU32::new(0),
            notified: AtomicBool::new(false),
            offline: AtomicBool::new(false),
            active: AtomicBool::new(false),
        })
    }
    fn new_main_thread<'a>(
        host: HostMainThreadHandle<'a>,
        shared: &'a Shared<'a>,
    ) -> Result<Main<'a>, PluginError> {
        Ok(Main { shared, host })
    }
}
impl<'a> PluginShared<'a> for Shared<'a> {}
impl<'a> PluginMainThread<'a, Shared<'a>> for Main<'a> {}
impl<'a> PluginAudioProcessor<'a, Shared<'a>, Main<'a>> for Audio<'a> {
    fn activate(
        _: HostAudioProcessorHandle<'a>,
        main: &Main<'a>,
        shared: &'a Shared<'a>,
        config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        if thread::current().id() != shared.main
            || config.sample_rate != 48000.0
            || config.max_frames_count > 128
        {
            return Err(PluginError::Message("invalid fixture activation"));
        }
        let latency = shared.requested.load(Ordering::Acquire);
        let old = shared.applied.swap(latency, Ordering::AcqRel);
        shared.active.store(true, Ordering::Release);
        if old != latency
            && let Some(extension) = main.host.get_extension::<HostLatency>()
        {
            extension.changed(&main.host);
        }
        Ok(Self {
            shared,
            delay: [[0.0; 2]; 257],
            cursor: 0,
            elapsed: 0,
            expression: [127; 16],
            latency: latency as usize,
        })
    }
    fn deactivate(self, _: &Main) {
        assert_eq!(thread::current().id(), self.shared.main);
        self.shared.active.store(false, Ordering::Release);
    }
    fn start_processing(&mut self) -> Result<(), PluginError> {
        if thread::current().id() == self.shared.main {
            return Err(PluginError::Message("processing on main thread"));
        }
        Ok(())
    }
    fn stop_processing(&mut self) {
        assert_ne!(thread::current().id(), self.shared.main);
    }
    fn process(
        &mut self,
        _: Process,
        mut audio: clack_plugin::process::Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        if thread::current().id() == self.shared.main {
            return Err(PluginError::Message("process on main thread"));
        }
        let mut input = [[0.0; 2]; 128];
        if MULTI {
            // The sidechain must arrive silent; every auxiliary output is written so a host
            // that mixes them in is caught.
            let port = audio
                .input_port(0)
                .ok_or(PluginError::Message("missing sidechain"))?;
            let channels = port
                .channels()?
                .into_f32()
                .ok_or(PluginError::Message("expected f32"))?;
            for channel in 0..2 {
                let samples = channels
                    .channel(channel)
                    .ok_or(PluginError::Message("missing sidechain channel"))?;
                if samples.iter().any(|&sample| sample != 0.0) {
                    return Err(PluginError::Message("sidechain is not silent"));
                }
            }
            for index in 1..OUTPUTS {
                let mut port = audio
                    .output_port(index as usize)
                    .ok_or(PluginError::Message("missing auxiliary output"))?;
                let mut channels = port
                    .channels()?
                    .into_f32()
                    .ok_or(PluginError::Message("expected f32"))?;
                for channel in 0..2 {
                    channels
                        .channel_mut(channel)
                        .ok_or(PluginError::Message("missing auxiliary channel"))?
                        .fill(AUX_LEVEL);
                }
            }
        }
        if EFFECT {
            let port = audio
                .input_port(0)
                .ok_or(PluginError::Message("missing effect input"))?;
            let channels = port
                .channels()?
                .into_f32()
                .ok_or(PluginError::Message("expected f32"))?;
            for channel in 0..2 {
                let samples = channels
                    .channel(channel)
                    .ok_or(PluginError::Message("missing effect channel"))?;
                for (frame, &sample) in input.iter_mut().zip(samples) {
                    frame[channel as usize] = sample * 0.5;
                }
            }
        }
        let mut port = audio
            .output_port(0)
            .ok_or(PluginError::Message("missing output"))?;
        let mut channels = port
            .channels()?
            .into_f32()
            .ok_or(PluginError::Message("expected f32"))?;
        let count = channels
            .channel_mut(0)
            .ok_or(PluginError::Message("missing left"))?
            .len();
        if count > 128 {
            return Err(PluginError::Message("oversized block"));
        }
        for event in events.input {
            if let Some(CoreEventSpace::Midi(midi)) = event.as_core_event() {
                let data = midi.data();
                let channel = usize::from(data[0] & 0x0f);
                if data[0] & 0xf0 == 0xb0 && data[1] == 11 {
                    self.expression[channel] = data[2];
                }
                if data[0] & 0xf0 == 0x90 && data[2] > 0 {
                    let offset = event.header().time() as usize;
                    if offset >= count {
                        return Err(PluginError::Message("invalid MIDI offset"));
                    }
                    input[offset] = [0.25 * f32::from(self.expression[channel]) / 127.0; 2];
                }
            }
        }
        let mut output = [[0.0; 2]; 128];
        for (i, &sample) in input[..count].iter().enumerate() {
            output[i] = if self.latency == 0 {
                sample
            } else {
                let old = self.delay[self.cursor];
                self.delay[self.cursor] = sample;
                self.cursor = (self.cursor + 1) % self.latency;
                old
            };
        }
        for channel in 0..if MONO_MAIN { 1 } else { 2 } {
            let destination = channels
                .channel_mut(channel)
                .ok_or(PluginError::Message("missing output channel"))?;
            for (sample, frame) in destination.iter_mut().zip(&output[..count]) {
                *sample = frame[channel as usize];
            }
        }
        self.elapsed += count as u64;
        if self.elapsed >= 48_000
            && !self.shared.offline.load(Ordering::Acquire)
            && !self.shared.notified.swap(true, Ordering::AcqRel)
        {
            self.shared.requested.store(257, Ordering::Release);
            self.shared.host.request_restart();
        }
        Ok(ProcessStatus::Continue)
    }
}
impl PluginAudioPortsImpl for Main<'_> {
    fn count(&self, input: bool) -> u32 {
        if input {
            u32::from(EFFECT || MULTI)
        } else {
            OUTPUTS
        }
    }
    fn get(&self, index: u32, input: bool, writer: &mut AudioPortInfoWriter) {
        if MULTI {
            let main = !input && index == 0;
            if index < if input { 1 } else { OUTPUTS } {
                writer.set(&AudioPortInfo {
                    id: ClapId::new(index + if input { 100 } else { 0 }),
                    name: if main {
                        b"Main"
                    } else if input {
                        b"Sidechain"
                    } else {
                        b"Aux"
                    },
                    channel_count: if main && MONO_MAIN { 1 } else { 2 },
                    flags: if main {
                        AudioPortFlags::IS_MAIN
                    } else {
                        AudioPortFlags::empty()
                    },
                    port_type: Some(if main && MONO_MAIN {
                        AudioPortType::MONO
                    } else {
                        AudioPortType::STEREO
                    }),
                    in_place_pair: None,
                });
            }
            return;
        }
        if index == 0 && (!input || EFFECT) {
            writer.set(&AudioPortInfo {
                id: ClapId::new(0),
                name: b"Stereo",
                channel_count: 2,
                flags: AudioPortFlags::IS_MAIN,
                port_type: Some(AudioPortType::STEREO),
                in_place_pair: None,
            });
        }
    }
}
impl PluginNotePortsImpl for Main<'_> {
    fn count(&self, input: bool) -> u32 {
        u32::from(input && !EFFECT)
    }
    fn get(&self, index: u32, input: bool, writer: &mut NotePortInfoWriter) {
        if index == 0 && input && !EFFECT {
            writer.set(&NotePortInfo {
                id: ClapId::new(0),
                name: b"MIDI",
                supported_dialects: NoteDialects::MIDI,
                preferred_dialect: Some(NoteDialect::Midi),
            });
        }
    }
}
impl PluginLatencyImpl for Main<'_> {
    fn get(&self) -> u32 {
        assert_eq!(thread::current().id(), self.shared.main);
        assert!(
            self.shared.active.load(Ordering::Acquire),
            "inactive latency query"
        );
        self.shared.applied.load(Ordering::Acquire)
    }
}
impl PluginRenderImpl for Main<'_> {
    fn has_hard_realtime_requirement(&self) -> bool {
        false
    }
    fn set(&self, mode: RenderMode) -> Result<(), PluginError> {
        self.shared
            .offline
            .store(mode == RenderMode::Offline, Ordering::Release);
        Ok(())
    }
}
impl PluginStateImpl for Main<'_> {
    fn save(&self, output: &mut OutputStream) -> Result<(), PluginError> {
        output.write_all(&self.shared.requested.load(Ordering::Acquire).to_le_bytes())?;
        Ok(())
    }
    fn load(&self, input: &mut InputStream) -> Result<(), PluginError> {
        if self.shared.active.load(Ordering::Acquire) {
            return Err(PluginError::Message("active restore"));
        }
        let mut bytes = [0; 4];
        input.read_exact(&mut bytes)?;
        let value = u32::from_le_bytes(bytes);
        if ![0, 257].contains(&value) {
            return Err(PluginError::Message("invalid latency state"));
        }
        self.shared.requested.store(value, Ordering::Release);
        Ok(())
    }
}
clack_export_entry!(SinglePluginEntry<Fixture>);
