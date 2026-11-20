use super::{Edit, Session};
use clef_model::{
    Result,
    mixer::{Control, Target},
};
use std::{
    collections::BTreeMap,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meter {
    pub level: [i16; 2],
    pub hold: [i16; 2],
    pub clipped: bool,
}

impl Default for Meter {
    fn default() -> Self {
        Self {
            level: [-600; 2],
            hold: [-600; 2],
            clipped: false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct MixChange {
    pub epoch: u64,
    pub target: Target,
    pub control: Control,
    pub value: i16,
}

#[derive(Default)]
struct Reading {
    meter: Meter,
    hold_until: [Option<Instant>; 2],
}

impl Reading {
    fn sample(&mut self, now: Instant, drop: i16, peak: [f32; 2]) -> Meter {
        for (i, peak) in peak.into_iter().enumerate() {
            let db = (200.0 * peak.log10()).round().clamp(-600.0, 60.0) as i16;
            let level = &mut self.meter.level[i];
            *level = db.max(level.saturating_sub(drop));
            self.meter.clipped |= peak > 1.0;
            let hold = &mut self.meter.hold[i];
            if db >= *hold {
                *hold = db;
                self.hold_until[i] = Some(now + Duration::from_secs(1));
            } else if self.hold_until[i].is_none_or(|until| now >= until) {
                *hold = (*level).max(hold.saturating_sub(drop));
            }
        }
        self.meter
    }
}

#[derive(Default)]
pub(super) struct Mixer {
    preview: Option<MixChange>,
    tracks: BTreeMap<u64, Reading>,
    master: Reading,
    last_sample: Option<Instant>,
}

impl Session {
    pub(super) fn publish_mix(&mut self) {
        if let Some(engine) = &mut self.engine {
            let preview = self
                .mixer
                .preview
                .filter(|preview| preview.epoch == self.view.project_epoch)
                .map(|p| (p.target, p.control, p.value));
            // A send drag is applied to the published mix only; the project keeps its level.
            let send = self
                .send_preview
                .filter(|preview| preview.epoch == self.view.project_epoch)
                .and_then(|preview| {
                    let slot = self
                        .view
                        .project
                        .tracks
                        .iter_mut()
                        .find(|track| track.id == preview.track)?
                        .sends
                        .iter_mut()
                        .find(|send| send.id == preview.id)?;
                    Some((preview, std::mem::replace(&mut slot.level, preview.level)))
                });
            engine.set_mix(&self.view.project, preview);
            if let Some((preview, level)) = send
                && let Some(slot) = self
                    .view
                    .project
                    .tracks
                    .iter_mut()
                    .find(|track| track.id == preview.track)
                    .and_then(|track| track.sends.iter_mut().find(|send| send.id == preview.id))
            {
                slot.level = level;
            }
        }
    }

    pub(super) fn preview_mix(&mut self, change: Option<MixChange>) -> Result<()> {
        if let Some(p) = change {
            if p.epoch != self.view.project_epoch {
                return Ok(());
            }
            if matches!(p.control, Control::Mute | Control::Solo) || !p.target.accepts(p.control) {
                return Err("Mixer control does not support preview.".into());
            }
            self.view.project.mix(p.target)?.set(p.control, p.value)?;
        }
        self.mixer.preview = change;
        Ok(())
    }

    pub(super) fn commit_mix(&mut self, change: MixChange) -> Result<()> {
        self.mixer.preview = None;
        let MixChange {
            epoch,
            target,
            control,
            value,
        } = change;
        if !self.view.project_open || epoch != self.view.project_epoch {
            return Err("The project changed before the mixer edit was applied.".into());
        }
        let before = self.view.project.mix(target)?.get(control);
        self.view.project.set_mix(target, control, value)?;
        if before != value {
            self.record_edit(Edit::Mix {
                target,
                control,
                before,
                after: value,
            });
        }
        Ok(())
    }

    pub(super) fn reset_meter(&mut self, target: Target) {
        let Some(engine) = &self.engine else { return };
        match target {
            Target::Master => {
                engine.clock.take_master_meter();
                self.mixer.master.meter.clipped = false;
                self.view.master_meter.clipped = false;
            }
            Target::Track(id) | Target::Bus(id) => {
                for track in &engine.clock.tracks {
                    if track.id.load(Ordering::Acquire) == id {
                        track.take_meter();
                    }
                }
                if matches!(target, Target::Bus(_)) {
                    let _ = engine.bus_meters();
                }
                if let Some(reading) = self.mixer.tracks.get_mut(&id) {
                    reading.meter.clipped = false;
                }
                if let Some(meter) = self.view.track_meters.get_mut(&id) {
                    meter.clipped = false;
                }
            }
        }
    }

    pub(super) fn update_meters(&mut self) {
        let Some(engine) = &self.engine else {
            self.mixer.tracks.clear();
            self.mixer.master = Reading::default();
            self.mixer.last_sample = None;
            self.view.track_meters.clear();
            self.view.master_meter = Meter::default();
            return;
        };
        let now = Instant::now();
        let elapsed = self
            .mixer
            .last_sample
            .map_or(Duration::from_millis(30), |last| now - last);
        if elapsed < Duration::from_millis(25) {
            return;
        }
        self.mixer.last_sample = Some(now);
        let drop = (elapsed.as_secs_f32() * 200.0).round() as i16;
        self.view.master_meter =
            self.mixer
                .master
                .sample(now, drop, engine.clock.take_master_meter());
        let mut current = BTreeMap::new();
        for track in &engine.clock.tracks {
            let id = track.id.load(Ordering::Acquire);
            if id != 0 {
                current.insert(id, track.take_meter());
            }
        }
        for (id, peak) in engine.bus_meters() {
            current.insert(id, peak);
        }
        self.mixer
            .tracks
            .retain(|id, _| self.view.project.tracks.iter().any(|track| track.id == *id));
        self.view.track_meters.clear();
        for track in &self.view.project.tracks {
            let reading = self.mixer.tracks.entry(track.id).or_default();
            let meter = reading.sample(now, drop, current.remove(&track.id).unwrap_or_default());
            if meter != Meter::default() {
                self.view.track_meters.insert(track.id, meter);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_decays_holds_and_latches_until_reset() {
        let mut reading = Reading::default();
        let now = Instant::now();
        let initial = reading.sample(now, 0, [2.0, 0.5]);
        assert_eq!(initial.level, [60, -60]);
        assert_eq!(initial.level, initial.hold);
        assert!(initial.clipped);
        let decay = reading.sample(now + Duration::from_millis(500), 100, [0.0; 2]);
        assert_eq!(decay.level, [-40, -160]);
        assert_eq!(decay.hold, initial.hold);
        assert!(decay.clipped);
        reading.meter.clipped = false;
        let released = reading.sample(now + Duration::from_millis(1010), 102, [0.0; 2]);
        assert_eq!(released.hold, [-42, -162]);
        assert!(!released.clipped);
    }
}
