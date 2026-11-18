use clef_model::{Result, project::AudioSettings};
pub use cpal::SupportedBufferSize;
use cpal::traits::{DeviceTrait, HostTrait};

pub fn buffer_supported(supported: &cpal::SupportedBufferSize, frames: u32) -> bool {
    frames > 0
        && match supported {
            cpal::SupportedBufferSize::Range { min, max } => (*min..=*max).contains(&frames),
            cpal::SupportedBufferSize::Unknown => true,
        }
}

pub struct Configuration {
    pub device: cpal::Device,
    pub pair: [usize; 2],
    pub midi_index: Option<usize>,
    pub midi_id: Option<String>,
    pub rate: u32,
    pub buffer: u32,
}

#[derive(Clone, Default)]
pub struct Devices {
    pub audio: Vec<AudioDevice>,
    pub midi: Vec<(String, String)>,
}
#[derive(Clone)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub configurations: Vec<AudioRange>,
}
#[derive(Clone)]
pub struct AudioRange {
    pub channels: u16,
    pub min_rate: u32,
    pub max_rate: u32,
    pub buffer: cpal::SupportedBufferSize,
}
impl AudioRange {
    pub fn supports(&self, settings: &AudioSettings) -> bool {
        (self.min_rate..=self.max_rate).contains(&settings.rate)
            && settings.pair.iter().all(|n| *n < self.channels as usize)
            && buffer_supported(&self.buffer, settings.buffer)
    }
}
impl Devices {
    pub fn enumerate() -> Result<Self> {
        let mut audio = Vec::new();
        for device in cpal::default_host().output_devices()? {
            let configurations = device
                .supported_output_configs()?
                .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
                .map(|c| AudioRange {
                    channels: c.channels(),
                    min_rate: c.min_sample_rate(),
                    max_rate: c.max_sample_rate(),
                    buffer: *c.buffer_size(),
                })
                .collect();
            audio.push(AudioDevice {
                id: device.id()?.to_string(),
                name: device.description()?.name().into(),
                configurations,
            });
        }
        let input = midir::MidiInput::new("Clef device list")?;
        let midi = input
            .ports()
            .iter()
            .map(|p| Ok((p.id(), input.port_name(p)?)))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { audio, midi })
    }
    pub fn validate(&self, settings: &AudioSettings) -> Result<()> {
        let device = self
            .audio
            .iter()
            .find(|d| d.id == settings.device)
            .ok_or("audio device unavailable; choose it in Audio/MIDI Settings")?;
        if !device.configurations.iter().any(|c| c.supports(settings)) {
            return Err("unsupported sample rate, stereo pair or buffer; choose supported values in Settings".into());
        }
        if !self.midi.iter().any(|(id, _)| id == &settings.midi) {
            return Err("MIDI input unavailable; choose it in Audio/MIDI Settings".into());
        }
        Ok(())
    }
}

pub fn print_devices() -> Result<()> {
    for (index, device) in cpal::default_host().output_devices()?.enumerate() {
        println!("Audio output {index}: {}", device.description()?);
        for config in device.supported_output_configs()? {
            println!("  {config:?}");
        }
    }
    let midi = midir::MidiInput::new("Clef T02 device enumeration")?;
    for (index, port) in midi.ports().iter().enumerate() {
        println!("MIDI input {index}: {}", midi.port_name(port)?);
    }
    Ok(())
}

pub fn output_by_index(index: usize) -> Result<cpal::Device> {
    cpal::default_host()
        .output_devices()?
        .nth(index)
        .ok_or_else(|| "audio output index unavailable; enumerate devices again".into())
}

pub fn output_by_id(id: &str) -> Result<cpal::Device> {
    cpal::default_host()
        .device_by_id(&id.parse()?)
        .ok_or_else(|| "audio device disconnected".into())
}

pub use cpal::Stream;
pub use cpal::traits::StreamTrait;
pub use midir::MidiInputConnection;

pub fn output_channels(
    device: &cpal::Device,
    pair: [usize; 2],
    rate: u32,
    buffer: u32,
) -> Result<Option<u16>> {
    Ok(device
        .supported_output_configs()?
        .find(|config| {
            config.sample_format() == cpal::SampleFormat::F32
                && config.min_sample_rate() <= rate
                && config.max_sample_rate() >= rate
                && buffer_supported(config.buffer_size(), buffer)
                && pair
                    .iter()
                    .all(|channel| *channel < usize::from(config.channels()))
        })
        .map(|config| config.channels()))
}

pub fn describe_output(device: &cpal::Device) -> Result<cpal::DeviceDescription> {
    Ok(device.description()?)
}

pub fn output_stream(
    device: &cpal::Device,
    channels: u16,
    rate: u32,
    buffer: u32,
    mut fill: impl FnMut(&mut [f32]) + Send + 'static,
    mut error: impl FnMut() + Send + 'static,
) -> Result<Stream> {
    Ok(device.build_output_stream(
        cpal::StreamConfig {
            channels,
            sample_rate: rate,
            buffer_size: cpal::BufferSize::Fixed(buffer),
        },
        move |data, _| fill(data),
        move |_| error(),
        None,
    )?)
}

pub fn midi_input(
    client: &str,
    connection: &str,
    id: Option<&str>,
    index: Option<usize>,
    announce: bool,
    mut receive: impl FnMut(&[u8]) + Send + 'static,
) -> Result<MidiInputConnection<()>> {
    let mut input = midir::MidiInput::new(client)?;
    input.ignore(midir::Ignore::None);
    let ports = input.ports();
    let port = if let Some(id) = id {
        ports.into_iter().find(|port| port.id() == id)
    } else {
        index.and_then(|index| ports.get(index).cloned())
    };
    let port = port.ok_or(if announce {
        "selected MIDI input unavailable"
    } else {
        "MIDI input unavailable"
    })?;
    if announce {
        println!("MIDI input: {}", input.port_name(&port)?);
    }
    Ok(input.connect(&port, connection, move |_, bytes, _| receive(bytes), ())?)
}
