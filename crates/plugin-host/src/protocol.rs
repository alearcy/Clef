use serde::{Deserialize, Serialize};

pub use clef_model::{DEFAULT_BLOCK_SIZE as BLOCK, DEFAULT_SAMPLE_RATE as RATE, MAX_STATE, Result};
pub const MAX_EVENTS: usize = 1024;
pub type Frame = [f32; 2];

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Midi {
    pub offset: u32,
    pub bytes: [u8; 3],
}

impl Midi {
    pub fn validate(self, frames: usize) -> Result<()> {
        let status = self.bytes[0];
        if self.offset as usize >= frames
            || !(0x80..=0xef).contains(&status)
            || self.bytes[1] > 127
            || self.bytes[2] > 127
        {
            return Err("invalid MIDI event or block offset".into());
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    Start {
        offline: bool,
    },
    Process {
        position: u64,
        events: Vec<Midi>,
        frames: usize,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        input: Vec<Frame>,
        /// Echoed in the reply so a late reply can be recognized and discarded.
        #[serde(default)]
        sequence: u64,
    },
    Stop,
    SuspendAudio,
    Editor {
        open: bool,
    },
    Save,
    FixtureEdit {
        value: u8,
    },
    FixtureFault {
        hang: bool,
    },
    /// Delay every following fixture process block by this many microseconds.
    FixtureDelay {
        micros: u32,
    },
    /// Make the fixture request a processing restart in its next reply.
    FixtureRestart,
    Restore {
        state: Vec<u8>,
    },
    Quit,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Reply {
    pub error: Option<String>,
    pub name: String,
    pub latency: u32,
    pub frames: Vec<Frame>,
    pub state: Vec<u8>,
    pub restart: bool,
    #[serde(default)]
    pub processing_nanos: u64,
    /// The `Process` request sequence this reply answers; zero for other replies.
    #[serde(default)]
    pub sequence: u64,
}

impl Reply {
    pub fn empty() -> Self {
        Self {
            error: None,
            name: String::new(),
            latency: 0,
            frames: Vec::new(),
            state: Vec::new(),
            restart: false,
            processing_nanos: 0,
            sequence: 0,
        }
    }

    pub fn failed(error: impl std::fmt::Display) -> Self {
        Self {
            error: Some(error.to_string()),
            ..Self::empty()
        }
    }
}

pub fn validate_block(frames: usize, events: &[Midi]) -> Result<()> {
    if frames == 0 || frames > BLOCK || events.len() > MAX_EVENTS {
        return Err("block or event count exceeds the prototype limit".into());
    }
    for &event in events {
        event.validate(frames)?;
    }
    if events
        .windows(2)
        .any(|pair| pair[0].offset > pair[1].offset)
    {
        return Err("MIDI events must be ordered by sample offset".into());
    }
    Ok(())
}

pub fn panic_events() -> Vec<Midi> {
    (0..16)
        .flat_map(|channel| {
            [
                Midi {
                    offset: 0,
                    bytes: [0xb0 | channel, 64, 0],
                },
                Midi {
                    offset: 0,
                    bytes: [0xb0 | channel, 123, 0],
                },
                Midi {
                    offset: 0,
                    bytes: [0xb0 | channel, 120, 0],
                },
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_rejects_invalid_shapes_and_offsets() {
        let note = Midi {
            offset: 127,
            bytes: [0x90, 60, 100],
        };
        assert!(validate_block(128, &[note]).is_ok());
        assert!(validate_block(127, &[note]).is_err());
        assert!(validate_block(129, &[]).is_err());
        assert!(
            validate_block(
                128,
                &[Midi {
                    offset: 0,
                    bytes: [0x90, 128, 0]
                }]
            )
            .is_err()
        );
        assert!(validate_block(128, &[note, Midi { offset: 0, ..note }]).is_err());
    }
}
