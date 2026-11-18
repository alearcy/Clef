//! Deterministic impulse instrument with a real 0 -> 257 sample latency restart.
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
    delay: [f32; 257],
    cursor: usize,
    elapsed: u64,
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
        PluginDescriptor::new("dev.clef.latency-fixture", "Clef CLAP Latency Fixture")
            .with_features([
                clack_plugin::plugin::features::INSTRUMENT,
                clack_plugin::plugin::features::STEREO,
            ])
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
            delay: [0.0; 257],
            cursor: 0,
            elapsed: 0,
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
        let mut input = [0.0; 128];
        for event in events.input {
            if let Some(CoreEventSpace::Midi(midi)) = event.as_core_event() {
                let data = midi.data();
                if data[0] & 0xf0 == 0x90 && data[2] > 0 {
                    let offset = event.header().time() as usize;
                    if offset >= count {
                        return Err(PluginError::Message("invalid MIDI offset"));
                    }
                    input[offset] = 0.25;
                }
            }
        }
        let mut output = [0.0; 128];
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
        for channel in 0..2 {
            channels
                .channel_mut(channel)
                .ok_or(PluginError::Message("missing output channel"))?
                .copy_from_slice(&output[..count]);
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
        u32::from(!input)
    }
    fn get(&self, index: u32, input: bool, writer: &mut AudioPortInfoWriter) {
        if index == 0 && !input {
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
        u32::from(input)
    }
    fn get(&self, index: u32, input: bool, writer: &mut NotePortInfoWriter) {
        if index == 0 && input {
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
