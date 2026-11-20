use super::{Edit, Project, Result, Session};
use clef_engine::inserts::{Chain, Effect, Slot};
use clef_model::{Plugin, mixer::Target, project::Insert};
use clef_plugin_host::{quarantine, rpc::Instrument};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::AtomicBool},
};

pub enum InsertAction {
    Add { before: Option<u64>, plugin: Plugin },
    Remove(u64),
    Move { id: u64, before: Option<u64> },
    Bypass(u64),
    Editor(u64),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InsertView {
    pub error: String,
    /// Percent of late blocks while the running effect is Late (T37 L1); never a failure.
    pub late: Option<u8>,
    pub missing: bool,
    pub editor: bool,
    pub loss_version: Option<u64>,
}

#[derive(Default)]
pub(super) struct Runtime {
    worker: Option<Instrument>,
    active: Option<Arc<Effect>>,
    editor: bool,
    touched: bool,
    error: String,
    loss_version: Option<u64>,
}

fn chains(project: &Project) -> impl Iterator<Item = (Target, &[Insert])> {
    project
        .tracks
        .iter()
        .map(|track| {
            let target = if track.is_bus() {
                Target::Bus(track.id)
            } else {
                Target::Track(track.id)
            };
            (target, track.inserts.as_slice())
        })
        .chain([(Target::Master, project.master_inserts.as_slice())])
}

fn position(chain: &[Insert], id: u64) -> Result<usize> {
    chain
        .iter()
        .position(|insert| insert.id == id)
        .ok_or_else(|| "Insert slot is no longer available.".into())
}

fn insert_before(chain: &mut Vec<Insert>, before: Option<u64>, insert: Insert) -> Result<()> {
    let index = before
        .map(|id| position(chain, id))
        .transpose()?
        .unwrap_or(chain.len());
    chain.insert(index, insert);
    Ok(())
}

impl Session {
    pub(super) fn clear_insert_losses(&mut self) {
        for runtime in self.insert_workers.values_mut() {
            runtime.loss_version = None;
            runtime.touched = false;
        }
    }
    fn load_insert(&self, insert: &Insert) -> Result<Instrument> {
        if self.registry.as_os_str().is_empty() {
            return Err("Quarantine registry unavailable.".into());
        }
        if self.blocked(&insert.plugin)
            || quarantine::read(&self.registry)?
                .iter()
                .any(|entry| quarantine::same(&entry.plugin, &insert.plugin))
        {
            return Err("Effect is quarantined.".into());
        }
        if !insert.plugin.path.exists() {
            return Err("Effect is missing.".into());
        }
        Instrument::load_effect(
            &self.executable,
            &insert.plugin.path,
            &insert.plugin.id,
            self.view.project.audio.rate,
            (!insert.state.is_empty()).then_some(insert.state.as_ref()),
            &AtomicBool::new(false),
        )
    }

    fn snapshot_insert(runtime: &mut Runtime) -> Result<Option<Arc<[u8]>>> {
        if runtime.loss_version.is_some() {
            return Err(
                "The effect's latest state was lost. Save with retained state before removing it."
                    .into(),
            );
        }
        let Some(worker) = runtime.worker.as_mut() else {
            return Ok(None);
        };
        if runtime.editor {
            worker.editor(false)?;
            runtime.editor = false;
        }
        let state = if let Some(active) = &runtime.active {
            active.capture(worker)?
        } else {
            worker.save()?
        };
        Ok(Some(state.into()))
    }

    pub(super) fn capture_removed_inserts(&mut self, candidate: &Project) -> Result<()> {
        let retained: HashSet<_> = chains(candidate)
            .flat_map(|(_, chain)| chain.iter().map(|slot| slot.id))
            .collect();
        for slot in self
            .view
            .project
            .tracks
            .iter_mut()
            .flat_map(|track| &mut track.inserts)
            .chain(&mut self.view.project.master_inserts)
        {
            if !retained.contains(&slot.id)
                && let Some(runtime) = self.insert_workers.get_mut(&slot.id)
                && let Some(state) = Self::snapshot_insert(runtime)?
            {
                slot.state = state;
            }
        }
        Ok(())
    }

    pub(super) fn capture_inserts(
        &mut self,
        candidate: &mut Project,
        use_retained: &[(u64, u64)],
    ) -> Result<()> {
        for slot in candidate
            .tracks
            .iter_mut()
            .flat_map(|track| &mut track.inserts)
            .chain(&mut candidate.master_inserts)
        {
            if let Some(runtime) = self.insert_workers.get_mut(&slot.id) {
                if runtime
                    .loss_version
                    .is_some_and(|version| use_retained.contains(&(slot.id, version)))
                {
                    continue;
                }
                if let Some(state) = Self::snapshot_insert(runtime)? {
                    slot.state = state;
                }
            }
        }
        Ok(())
    }

    pub(super) fn stop_inserts(&mut self) {
        for runtime in self.insert_workers.values_mut() {
            if let Some((active, worker)) = runtime.active.take().zip(runtime.worker.as_mut())
                && let Err(error) = active.deactivate(worker)
            {
                runtime.error = error.to_string();
                runtime.worker = None;
                runtime.editor = false;
                runtime.loss_version = runtime.touched.then_some(self.insert_loss_version);
                self.insert_loss_version = self.insert_loss_version.wrapping_add(1);
            }
        }
    }

    /// Prepare workers and validate all affected chains before publishing project edits.
    pub(super) fn reconcile_inserts(&mut self, project: &Project, all: bool) -> Result<()> {
        let mut added = HashMap::<u64, Runtime>::new();
        let mut updates = Vec::new();
        let mut keep = HashSet::new();
        for (target, chain) in chains(project) {
            keep.extend(chain.iter().map(|slot| slot.id));
            let offline = match target {
                Target::Master | Target::Bus(_) => false,
                Target::Track(id) => project
                    .tracks
                    .iter()
                    .find(|track| track.id == id)
                    .is_some_and(|track| track.instrument_offline),
            };
            let was_offline = match target {
                Target::Master | Target::Bus(_) => false,
                Target::Track(id) => self
                    .view
                    .project
                    .tracks
                    .iter()
                    .find(|track| track.id == id)
                    .is_some_and(|track| track.instrument_offline),
            };
            if offline {
                if self.engine.is_some() {
                    updates.push((target, None));
                }
                continue;
            }
            let prior = self.view.project.inserts(target).unwrap_or(&[]);
            let same = chain
                .iter()
                .map(|s| (s.id, s.bypassed))
                .eq(prior.iter().map(|s| (s.id, s.bypassed)));
            if self.engine.is_none() || (!all && same && !was_offline) {
                continue;
            }
            let mut slots = Vec::new();
            for insert in chain {
                let active = if let Some(runtime) = self.insert_workers.get_mut(&insert.id) {
                    if runtime.active.is_none()
                        && runtime.error.is_empty()
                        && let Some(worker) = &mut runtime.worker
                    {
                        runtime.active = Some(Effect::activate(worker)?);
                    }
                    runtime.active.clone()
                } else {
                    let mut runtime = Runtime::default();
                    match self.load_insert(insert).and_then(|mut worker| {
                        let active = Effect::activate(&mut worker)?;
                        Ok((worker, active))
                    }) {
                        Ok((worker, active)) => {
                            runtime.worker = Some(worker);
                            runtime.active = Some(active);
                        }
                        Err(error) => runtime.error = error.to_string(),
                    }
                    let active = runtime.active.clone();
                    added.insert(insert.id, runtime);
                    active
                };
                if let Some(effect) = active {
                    slots.push(Slot {
                        effect,
                        bypassed: insert.bypassed,
                    });
                }
            }
            let chain = if slots.is_empty() {
                None
            } else {
                Some(Chain::prepare(slots)?)
            };
            updates.push((target, chain));
        }
        if let Some(engine) = &mut self.engine {
            for track in &self.view.project.tracks {
                let target = if track.is_bus() {
                    Target::Bus(track.id)
                } else {
                    Target::Track(track.id)
                };
                if project.inserts(target).is_none() && !track.inserts.is_empty() {
                    engine.set_insert_chain(target, None);
                }
            }
            for (target, chain) in updates {
                engine.set_insert_chain(target, chain);
            }
        }
        self.insert_workers.extend(added);
        self.insert_workers.retain(|id, _| keep.contains(id));
        Ok(())
    }

    pub(super) fn edit_inserts(
        &mut self,
        epoch: u64,
        target: Target,
        action: InsertAction,
    ) -> Result<()> {
        if epoch != self.view.project_epoch || !self.view.project_open {
            return Err("The mixer channel is no longer available.".into());
        }
        let prior = self
            .view
            .project
            .inserts(target)
            .ok_or("The mixer channel is no longer available.")?;
        if let InsertAction::Editor(id) = action {
            let insert = prior[position(prior, id)?].clone();
            if !self.insert_workers.contains_key(&id) {
                let worker = self.load_insert(&insert)?;
                self.insert_workers.insert(
                    id,
                    Runtime {
                        worker: Some(worker),
                        ..Default::default()
                    },
                );
            }
            let runtime = self.insert_workers.get_mut(&id).expect("insert runtime");
            if !runtime.error.is_empty() {
                return Err(runtime.error.clone().into());
            }
            let worker = runtime
                .worker
                .as_mut()
                .ok_or("Effect editor is unavailable.")?;
            worker.editor(true)?;
            runtime.editor = true;
            runtime.touched = true;
            self.mark_other_change();
            return Ok(());
        }
        let mut before = prior.to_vec();
        let mut after = before.clone();
        let mut candidate = self.view.project.clone();
        let mut created = None;
        let label = match action {
            InsertAction::Add {
                before: anchor,
                plugin,
            } => {
                let id = candidate.next_id;
                candidate.next_id = id
                    .checked_add(1)
                    .ok_or("Insert identity space exhausted.")?;
                let mut insert = Insert {
                    id,
                    plugin,
                    bypassed: false,
                    state: Arc::default(),
                };
                let mut worker = self.load_insert(&insert)?;
                insert.state = worker.save()?.into();
                let active = if self.engine.is_some() {
                    Some(Effect::activate(&mut worker)?)
                } else {
                    None
                };
                created = Some((
                    id,
                    Runtime {
                        worker: Some(worker),
                        active,
                        ..Default::default()
                    },
                ));
                insert_before(&mut after, anchor, insert)?;
                "Add Insert Effect"
            }
            InsertAction::Remove(id) => {
                let index = position(&after, id)?;
                if let Some(runtime) = self.insert_workers.get_mut(&id)
                    && let Some(state) = Self::snapshot_insert(runtime)?
                {
                    before[index].state = state;
                }
                after.remove(index);
                "Remove Insert Effect"
            }
            InsertAction::Move { id, before } => {
                if before == Some(id) {
                    return Ok(());
                }
                let index = position(&after, id)?;
                let insert = after.remove(index);
                insert_before(&mut after, before, insert)?;
                "Move Insert Effect"
            }
            InsertAction::Bypass(id) => {
                let index = position(&after, id)?;
                after[index].bypassed = !after[index].bypassed;
                "Bypass Insert Effect"
            }
            InsertAction::Editor(_) => unreachable!(),
        };
        if before == after {
            return Ok(());
        }
        *candidate
            .inserts_mut(target)
            .ok_or("The mixer channel is no longer available.")? = after.clone();
        candidate.validate_storage()?;
        let created = created.map(|(id, runtime)| {
            self.insert_workers.insert(id, runtime);
            id
        });
        if let Err(error) = self.reconcile_inserts(&candidate, false) {
            if let Some(id) = created {
                self.insert_workers.remove(&id);
            }
            return Err(error);
        }
        self.view.project = candidate;
        self.record_edit(Edit::Inserts {
            target,
            before,
            after,
            label,
        });
        Ok(())
    }

    pub(super) fn poll_inserts(&mut self) {
        let mut faults = Vec::new();
        for (&id, runtime) in &mut self.insert_workers {
            let error = runtime
                .active
                .as_ref()
                .and_then(|effect| effect.error())
                .or_else(|| {
                    runtime
                        .worker
                        .as_mut()
                        .is_some_and(Instrument::exited)
                        .then(|| "Effect worker exited.".to_owned())
                });
            if let Some(error) = error.filter(|_| runtime.error.is_empty()) {
                runtime.error = error;
                runtime.editor = false;
                runtime.loss_version = runtime.touched.then_some(self.insert_loss_version);
                self.insert_loss_version = self.insert_loss_version.wrapping_add(1);
                runtime.worker = None;
                faults.push(id);
            } else if runtime.editor
                && runtime
                    .worker
                    .as_mut()
                    .is_some_and(Instrument::editor_closed)
            {
                runtime.editor = false;
            }
        }
        for id in faults {
            let plugin = chains(&self.view.project)
                .flat_map(|(_, chain)| chain)
                .find(|s| s.id == id)
                .map(|insert| insert.plugin.clone());
            if let Some(plugin) = plugin {
                let reason = self.insert_workers[&id].error.clone();
                self.quarantine_kind(plugin, reason, true);
            }
        }
    }

    pub(super) fn refresh_insert_views(&mut self) {
        let missing: HashSet<u64> = chains(&self.view.project)
            .flat_map(|(_, chain)| chain)
            .filter(|insert| !self.paths.exists(&insert.plugin.path))
            .map(|insert| insert.id)
            .collect();
        self.view.inserts = chains(&self.view.project)
            .flat_map(|(_, chain)| chain)
            .map(|insert| {
                let runtime = self.insert_workers.get(&insert.id);
                let missing = missing.contains(&insert.id);
                let error = runtime
                    .map(|r| r.error.clone())
                    .filter(|s| !s.is_empty())
                    .or_else(|| {
                        self.blocked(&insert.plugin)
                            .then(|| "Effect is quarantined.".into())
                    })
                    .unwrap_or_default();
                let late = runtime
                    .and_then(|r| r.active.as_ref())
                    .and_then(|effect| effect.late_percent())
                    .filter(|_| error.is_empty() && !missing);
                (
                    insert.id,
                    InsertView {
                        error,
                        late,
                        missing,
                        editor: runtime.is_some_and(|r| r.editor),
                        loss_version: runtime.and_then(|r| r.loss_version),
                    },
                )
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Action;
    use clef_model::project::TemporaryDirectory;

    #[test]
    fn insertion_anchors_survive_reordering_and_reject_stale_slots() {
        let insert = |id| Insert {
            id,
            plugin: Plugin {
                id: id.to_string(),
                name: format!("Effect {id}"),
                format: "VST3".into(),
                path: "/missing/Effect.vst3".into(),
            },
            state: Arc::default(),
            bypassed: false,
        };
        let mut chain = vec![insert(1_u64), insert(2)];
        insert_before(&mut chain, Some(2), insert(3)).unwrap();
        assert_eq!(
            chain.iter().map(|slot| slot.id).collect::<Vec<_>>(),
            [1, 3, 2]
        );
        assert!(insert_before(&mut chain, Some(4), insert(5)).is_err());
        assert_eq!(chain.len(), 3);
    }

    #[test]
    #[ignore = "requires the bundled CLAP insert fixture and macOS WindowServer"]
    fn nine_inserts_and_master_survive_ordered_edits_undo_and_reopen() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = root
            .join("validation-output/fixtures/ClefInsertCLAP.clap")
            .canonicalize()
            .unwrap();
        let directory = TemporaryDirectory::new("insert-session").unwrap();
        let project = directory.0.join("Inserts.clef");
        let mut session = Session::with_registry(
            directory.0.join("quarantine.json"),
            root.join("target/debug/clef"),
        );
        session.apply(Action::New).unwrap();
        session.apply(Action::AddTrack(None)).unwrap();
        let track = Target::Track(session.view.selected.unwrap());
        let epoch = session.view.project_epoch;
        let plugin = Plugin {
            path,
            id: "dev.clef.insert-fixture".into(),
            name: "Clef Insert Fixture".into(),
            format: "CLAP".into(),
        };
        let add = |target, before| {
            Action::Insert(
                epoch,
                target,
                InsertAction::Add {
                    before,
                    plugin: plugin.clone(),
                },
            )
        };
        let ids = |session: &Session| {
            session
                .view
                .project
                .inserts(track)
                .unwrap()
                .iter()
                .map(|slot| slot.id)
                .collect::<Vec<_>>()
        };
        session.apply(add(track, None)).unwrap();
        session.apply(add(track, None)).unwrap();
        let ab = ids(&session);
        session.apply(add(track, Some(ab[1]))).unwrap();
        let c = ids(&session)[1];
        assert_eq!(ids(&session), [ab[0], c, ab[1]]);
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor - 1,
            })
            .unwrap();
        assert_eq!(ids(&session), ab);
        let redo = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: redo.version,
                target: redo.cursor + 1,
            })
            .unwrap();
        assert_eq!(ids(&session), [ab[0], c, ab[1]]);
        for _ in 0..6 {
            session.apply(add(track, None)).unwrap();
        }
        session.apply(add(Target::Master, None)).unwrap();
        session
            .apply(Action::Insert(
                epoch,
                track,
                InsertAction::Move {
                    id: c,
                    before: None,
                },
            ))
            .unwrap();
        session
            .apply(Action::Insert(epoch, track, InsertAction::Bypass(c)))
            .unwrap();
        let order = ids(&session);
        assert_eq!(order.len(), 9);
        assert_eq!(order.last(), Some(&c));
        assert!(
            session
                .view
                .project
                .inserts(track)
                .unwrap()
                .last()
                .unwrap()
                .bypassed
        );
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor - 1,
            })
            .unwrap();
        assert!(
            !session
                .view
                .project
                .inserts(track)
                .unwrap()
                .last()
                .unwrap()
                .bypassed
        );
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor + 1,
            })
            .unwrap();
        assert!(
            session
                .view
                .project
                .inserts(track)
                .unwrap()
                .last()
                .unwrap()
                .bypassed
        );

        let before_move = ids(&session);
        session
            .apply(Action::Insert(
                epoch,
                track,
                InsertAction::Move {
                    id: c,
                    before: Some(before_move[0]),
                },
            ))
            .unwrap();
        assert_eq!(ids(&session)[0], c);
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor - 1,
            })
            .unwrap();
        assert_eq!(ids(&session), before_move);
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor + 1,
            })
            .unwrap();
        assert_eq!(ids(&session)[0], c);
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor - 1,
            })
            .unwrap();
        assert_eq!(ids(&session), before_move);

        let victim = ids(&session)[1];
        session
            .apply(Action::Insert(epoch, track, InsertAction::Remove(victim)))
            .unwrap();
        assert_eq!(ids(&session).len(), 8);
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor - 1,
            })
            .unwrap();
        assert_eq!(ids(&session), before_move);
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor + 1,
            })
            .unwrap();
        assert_eq!(ids(&session).len(), 8);
        let history = session.view.edit_history.clone();
        session
            .apply(Action::SeekHistory {
                version: history.version,
                target: history.cursor - 1,
            })
            .unwrap();
        assert_eq!(ids(&session), before_move);
        let order = ids(&session);
        session
            .apply(Action::Save {
                path: project.clone(),
                replace: false,
                track_name: None,
                use_retained: vec![],
            })
            .unwrap();
        session.apply(Action::Open(project)).unwrap();
        assert_eq!(ids(&session), order);
        assert!(
            session
                .view
                .project
                .inserts(track)
                .unwrap()
                .last()
                .unwrap()
                .bypassed
        );
        assert_eq!(session.view.project.master_inserts.len(), 1);
        assert!(session.insert_workers.is_empty());
    }
}
