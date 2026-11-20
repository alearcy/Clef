use super::{Edit, Project, Result, Session};

impl Session {
    pub(super) fn sync_instrument_bypasses(&self, project: &Project) {
        if let Some(engine) = &self.engine {
            for track in &project.tracks {
                engine.set_instrument_bypass(track.id, track.instrument_bypassed);
            }
        }
    }

    pub(super) fn set_instrument_bypass(
        &mut self,
        epoch: u64,
        id: u64,
        bypassed: bool,
    ) -> Result<()> {
        if epoch != self.view.project_epoch || !self.view.project_open {
            return Err("The instrument track is no longer available.".into());
        }
        let track = self
            .view
            .project
            .tracks
            .iter()
            .find(|track| track.id == id)
            .ok_or("The instrument track is no longer available.")?;
        if track.plugin.is_none() {
            return Err("Choose an instrument before bypassing the slot.".into());
        }
        let before = track.instrument_bypassed;
        if before == bypassed {
            return Ok(());
        }
        let mut candidate = self.view.project.clone();
        let track = candidate
            .tracks
            .iter_mut()
            .find(|track| track.id == id)
            .ok_or("The instrument track is no longer available.")?;
        track.instrument_bypassed = bypassed;
        candidate.validate()?;
        if let Some(engine) = &self.engine {
            engine.set_instrument_bypass(id, bypassed);
        }
        self.view.project = candidate;
        self.record_edit(Edit::InstrumentBypass {
            id,
            before,
            after: bypassed,
        });
        self.view.message = if bypassed {
            "Instrument bypassed. Its output is muted and its editor remains available.".into()
        } else {
            "Instrument enabled.".into()
        };
        Ok(())
    }

    pub(super) fn set_instrument_offline(
        &mut self,
        epoch: u64,
        id: u64,
        offline: bool,
    ) -> Result<()> {
        if epoch != self.view.project_epoch || !self.view.project_open {
            return Err("The instrument track is no longer available.".into());
        }
        let track = self
            .view
            .project
            .tracks
            .iter()
            .find(|track| track.id == id)
            .ok_or("The instrument track is no longer available.")?;
        if track.plugin.is_none() {
            return Err("Choose an instrument before changing its online state.".into());
        }
        let before = track.instrument_offline;
        if before == offline {
            return Ok(());
        }
        if offline {
            if self.view.recording {
                return Err("Stop recording before taking an instrument offline.".into());
            }
            if self
                .runtimes
                .get(&id)
                .is_some_and(|runtime| runtime.loading)
            {
                return Err("Wait for the instrument load before taking it offline.".into());
            }
            if self.runtimes.get(&id).is_some_and(|runtime| runtime.editor) {
                self.set_track_editor(id, false)?;
            }
            let state = if self
                .runtimes
                .get(&id)
                .is_some_and(|runtime| runtime.instrument.is_some())
            {
                Some(self.capture_track_state(id)?)
            } else {
                None
            };
            let mut candidate = self.view.project.clone();
            if let Some(state) = state {
                candidate
                    .tracks
                    .iter_mut()
                    .find(|track| track.id == id)
                    .unwrap()
                    .state = state;
            }
            candidate
                .tracks
                .iter_mut()
                .find(|track| track.id == id)
                .unwrap()
                .instrument_offline = true;
            candidate.validate()?;
            self.reconcile_inserts(&candidate, false)?;
            self.view.project = candidate;
            self.release_track_worker(id);
            if self.view.armed == Some(id) {
                self.view.armed = None;
            }
            self.record_edit(Edit::InstrumentOffline {
                id,
                before,
                after: offline,
            });
            self.view.message =
                "Instrument offline. Its saved state is kept and processing is stopped.".into();
            return Ok(());
        }

        let mut candidate: Project = self.view.project.clone();
        candidate
            .tracks
            .iter_mut()
            .find(|track| track.id == id)
            .unwrap()
            .instrument_offline = false;
        candidate.validate()?;
        self.reconcile_inserts(&candidate, false)?;
        self.view.project = candidate;
        self.record_edit(Edit::InstrumentOffline {
            id,
            before,
            after: offline,
        });
        if self.engine.is_some()
            && self.live_target() == Some(id)
            && let Err(error) = self.request_load(id, None)
        {
            self.view.message = error.to_string();
            return Ok(());
        }
        self.view.message = "Instrument online. It loads when its path is next needed.".into();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Action;
    use clef_model::Plugin;

    fn session() -> Session {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut session = Session::with_registry(
            std::env::temp_dir().join(format!("clef-instrument-{}.json", std::process::id())),
            root.join("target/debug/clef"),
        );
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        session.view.project.tracks[0].plugin = Some(Plugin {
            path: "/missing/Instrument.vst3".into(),
            id: "missing".into(),
            name: "Instrument".into(),
            format: "VST3".into(),
        });
        session
    }

    fn seek(session: &mut Session, target: usize) -> Result<()> {
        let history = session.view.edit_history.clone();
        session.apply(Action::SeekHistory {
            version: history.version,
            target,
        })
    }

    #[test]
    fn bypass_and_offline_flags_are_undoable_and_offline_blocks_loading() {
        let mut session = session();
        let epoch = session.view.project_epoch;
        let id = session.view.project.tracks[0].id;
        assert!(!session.view.project.tracks[0].instrument_bypassed);
        session
            .apply(Action::InstrumentBypass {
                epoch,
                id,
                bypassed: true,
            })
            .unwrap();
        assert!(session.view.project.tracks[0].instrument_bypassed);
        let cursor = session.view.edit_history.cursor;
        seek(&mut session, cursor - 1).unwrap();
        assert!(!session.view.project.tracks[0].instrument_bypassed);
        seek(&mut session, cursor).unwrap();
        assert!(session.view.project.tracks[0].instrument_bypassed);

        let epoch = session.view.project_epoch;
        session
            .apply(Action::InstrumentOffline {
                epoch,
                id,
                offline: true,
            })
            .unwrap();
        assert!(session.view.project.tracks[0].instrument_offline);
        assert!(session.request_load(id, None).is_err());
        assert!(
            session
                .apply(Action::Select(
                    id,
                    Plugin {
                        path: "/missing/Other.vst3".into(),
                        id: "other".into(),
                        name: "Other".into(),
                        format: "VST3".into(),
                    }
                ))
                .is_err()
        );
        let cursor = session.view.edit_history.cursor;
        seek(&mut session, cursor - 1).unwrap();
        assert!(!session.view.project.tracks[0].instrument_offline);
        seek(&mut session, cursor).unwrap();
        assert!(session.view.project.tracks[0].instrument_offline);
        assert_eq!(session.view.tracks[0].status, "Offline");
    }

    #[test]
    #[ignore = "requires built Clef, TestSynth and macOS WindowServer"]
    fn offline_releases_the_instrument_worker_and_keeps_its_state() {
        use std::time::{Duration, Instant};

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut session = Session::with_registry(
            std::env::temp_dir().join(format!("clef-offline-{}.json", std::process::id())),
            root.join("target/debug/clef"),
        );
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        let id = session.view.project.tracks[0].id;
        session.view.project.tracks[0].plugin = Some(Plugin {
            path: root
                .join("vendor/rust-vst3-host/test_plugins/TestSynth.vst3")
                .canonicalize()
                .unwrap(),
            id: "5445535453594E5450524F4300000001".into(),
            name: "VST3 Host Test Synth".into(),
            format: "VST3".into(),
        });
        session.view.project.tracks[0].state = Vec::new();
        session.request_load(id, None).unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !session
            .runtimes
            .get(&id)
            .is_some_and(|runtime| runtime.instrument.is_some())
        {
            session.poll_jobs();
            assert!(Instant::now() < deadline, "instrument load timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
        let epoch = session.view.project_epoch;
        session
            .apply(Action::InstrumentOffline {
                epoch,
                id,
                offline: true,
            })
            .unwrap();
        assert!(session.view.project.tracks[0].instrument_offline);
        assert!(!session.view.project.tracks[0].state.is_empty());
        assert!(!session.runtimes.contains_key(&id));

        let cursor = session.view.edit_history.cursor;
        seek(&mut session, cursor - 1).unwrap();
        assert!(!session.view.project.tracks[0].instrument_offline);
        seek(&mut session, cursor).unwrap();
        assert!(session.view.project.tracks[0].instrument_offline);
    }
}
