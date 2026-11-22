use crate::Result;
use serde::{Deserialize, Serialize};

pub const QUARTER: u64 = 960;
pub const MAX_CHANGES: usize = 64;
pub const MAX_BAR: u32 = 16_384;
pub const MAX_TICK: u64 = MAX_BAR as u64 * 16 * 4 * QUARTER;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tempo {
    pub bar: u32,
    pub bpm: u16,
    pub beats: u8,
    pub unit: u8,
}

impl Tempo {
    pub fn beat_ticks(self) -> u64 {
        4 * QUARTER / u64::from(self.unit)
    }

    pub fn bar_ticks(self) -> u64 {
        self.beat_ticks() * u64::from(self.beats)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopMode {
    #[default]
    KeepPosition,
    ReturnToPlayStart,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Timeline {
    pub changes: Vec<Tempo>,
    pub loop_range: [u64; 2],
    pub looping: bool,
    pub click: bool,
    pub count_in: bool,
    pub stop_mode: StopMode,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            changes: vec![Tempo {
                bar: 1,
                bpm: 120,
                beats: 4,
                unit: 4,
            }],
            loop_range: [0, 16 * QUARTER],
            looping: false,
            click: false,
            count_in: false,
            stop_mode: StopMode::KeepPosition,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub bar: u32,
    pub beat: u8,
    pub tick: u64,
}

impl Timeline {
    pub fn validate(&self) -> Result<()> {
        if self.changes.first().is_none_or(|change| change.bar != 1)
            || self.changes.len() > MAX_CHANGES
            || self.changes.windows(2).any(|w| w[0].bar >= w[1].bar)
            || self.changes.iter().any(|change| {
                change.bar > MAX_BAR
                    || !(20..=400).contains(&change.bpm)
                    || !(1..=16).contains(&change.beats)
                    || !matches!(change.unit, 1 | 2 | 4 | 8 | 16 | 32)
            })
            || self.loop_range[0] >= self.loop_range[1]
            || self.loop_range[1] > MAX_TICK
        {
            return Err("invalid tempo map or loop range".into());
        }
        Ok(())
    }

    pub fn sections(&self) -> impl Iterator<Item = (u64, Tempo)> + '_ {
        self.changes.iter().copied().scan(None, |previous, tempo| {
            let tick = previous.map_or(0, |(tick, last): (u64, Tempo)| {
                tick + u64::from(tempo.bar - last.bar) * last.bar_ticks()
            });
            *previous = Some((tick, tempo));
            Some((tick, tempo))
        })
    }

    pub fn section_at(&self, tick: u64) -> (u64, Tempo) {
        self.sections()
            .take_while(|(start, _)| *start <= tick)
            .last()
            .expect("validated timeline starts at tick zero")
    }

    pub fn position(&self, tick: u64) -> Position {
        let (start, tempo) = self.section_at(tick);
        let relative = tick - start;
        Position {
            bar: tempo.bar + (relative / tempo.bar_ticks()) as u32,
            beat: (relative % tempo.bar_ticks() / tempo.beat_ticks()) as u8 + 1,
            tick: relative % tempo.beat_ticks(),
        }
    }

    /// T41: the time from the project start to `tick` in nanoseconds, through the tempo map
    /// (`bpm` counts quarter notes, as in the engine's sample map).
    pub fn nanos(&self, tick: u64) -> u128 {
        let tick = tick.min(MAX_TICK);
        let sections: Vec<_> = self.sections().collect();
        let mut total = 0u128;
        for (index, &(start, tempo)) in sections.iter().enumerate() {
            if start >= tick {
                break;
            }
            let end = sections
                .get(index + 1)
                .map_or(tick, |&(next, _)| next.min(tick));
            total += u128::from(end - start) * 60 * 1_000_000_000
                / (u128::from(tempo.bpm) * u128::from(QUARTER));
        }
        total
    }

    pub fn bar_start(&self, bar: u32) -> u64 {
        let bar = bar.max(1);
        let (start, tempo) = self
            .sections()
            .take_while(|(_, tempo)| tempo.bar <= bar)
            .last()
            .expect("validated timeline starts at bar one");
        (start + u64::from(bar - tempo.bar) * tempo.bar_ticks()).min(MAX_TICK)
    }

    pub fn nearest_beat(&self, tick: u64) -> u64 {
        let (start, tempo) = self.section_at(tick);
        let beat = tempo.beat_ticks();
        (start + ((tick - start + beat / 2) / beat) * beat).min(MAX_TICK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nanos_follow_the_tempo_map() {
        let mut timeline = Timeline::default();
        // 120 quarters per minute: one quarter is half a second.
        assert_eq!(timeline.nanos(0), 0);
        assert_eq!(timeline.nanos(QUARTER), 500_000_000);
        timeline.changes.push(Tempo {
            bar: 2,
            bpm: 60,
            beats: 4,
            unit: 4,
        });
        // Bar 1 (4 quarters at 120) is 2 s; one quarter at 60 adds 1 s.
        assert_eq!(timeline.nanos(5 * QUARTER), 3_000_000_000);
    }
}
