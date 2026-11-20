//! Persisted mixer controls. Levels and balance use fixed units for exact undo and save.
use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Level {
    pub fader: i16,
    pub pan: i16,
}

impl Level {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Channel {
    pub level: Level,
    pub trim: i16,
    pub width: i16,
    pub mute: bool,
    pub solo: bool,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            level: Level::default(),
            trim: 0,
            width: 100,
            mute: false,
            solo: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Gain,
    Pan,
    Width,
    Fader,
    Send,
    Mute,
    Solo,
}

impl Control {
    pub const fn bounds(self) -> (i16, i16) {
        match self {
            Self::Gain => (-240, 240),
            Self::Pan => (-100, 100),
            Self::Width => (0, 200),
            Self::Fader | Self::Send => (-601, 60),
            Self::Mute | Self::Solo => (0, 1),
        }
    }

    pub const fn initial(self) -> i16 {
        match self {
            Self::Width => 100,
            _ => 0,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Gain => "Gain",
            Self::Pan => "Pan",
            Self::Width => "Width",
            Self::Fader => "Fader",
            Self::Send => "Send",
            Self::Mute => "Mute",
            Self::Solo => "Solo",
        }
    }

    pub fn accepts(self, value: i16) -> bool {
        let (min, max) = self.bounds();
        (min..=max).contains(&value)
    }
}

impl Channel {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn get(self, control: Control) -> i16 {
        match control {
            Control::Gain => self.trim,
            Control::Pan => self.level.pan,
            Control::Width => self.width,
            Control::Fader => self.level.fader,
            Control::Send => 0,
            Control::Mute => i16::from(self.mute),
            Control::Solo => i16::from(self.solo),
        }
    }

    pub fn set(&mut self, control: Control, value: i16) -> Result<()> {
        if !control.accepts(value) {
            return Err("Mixer value is outside its supported range.".into());
        }
        match control {
            Control::Gain => self.trim = value,
            Control::Pan => self.level.pan = value,
            Control::Width => self.width = value,
            Control::Fader => self.level.fader = value,
            Control::Send => return Err("Send level is not a channel control.".into()),
            Control::Mute => self.mute = value != 0,
            Control::Solo => self.solo = value != 0,
        }
        Ok(())
    }

    pub fn valid(self) -> bool {
        [Control::Gain, Control::Width, Control::Fader, Control::Pan]
            .into_iter()
            .all(|control| control.accepts(self.get(control)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Track(u64),
    Bus(u64),
    Master,
}

impl Target {
    pub fn accepts(self, control: Control) -> bool {
        match self {
            Self::Track(_) | Self::Bus(_) => true,
            Self::Master => matches!(control, Control::Pan | Control::Fader),
        }
    }

    pub fn id(self) -> Option<u64> {
        match self {
            Self::Track(id) | Self::Bus(id) => Some(id),
            Self::Master => None,
        }
    }
}
