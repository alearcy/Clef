use super::{
    App, Command, Key, Modifiers, egui,
    primitives::{inserts, mixer as controls},
    theme::*,
};
use clef_model::mixer::{Channel, Control, Target};
use clef_session::{Action, InsertAction, Meter, MixChange};

#[derive(Default)]
pub(super) struct MixerUi {
    pub open: bool,
    master_selected: bool,
    focus: Option<Target>,
    gesture: Option<(MixChange, controls::DragOrigin)>,
    pending: Option<MixChange>,
    insert_focus: Option<(Target, u64)>,
    instrument_focus: Option<u64>,
    effect_picker: Option<(Target, Option<u64>)>,
    effect_search: String,
    visible_insert_rows: usize,
    pub(super) gallery_insert_focus: Option<u64>,
}

enum Event {
    Focus(Target),
    Adjust(Target, Control, controls::Adjustment),
    Command(Command, Target),
    FocusInsert(Target, u64),
    FocusInstrument(u64),
    Insert(Command, Target, Option<u64>),
    Instrument(Command, u64),
    DropInsert(Target, u64, Option<u64>),
}

struct DragInsert(Target, u64);

impl MixerUi {
    pub(super) fn follow_instrument_selection(&mut self) {
        self.master_selected = false;
    }

    pub(super) fn commit_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn acknowledge(&mut self, project_changed: bool) {
        self.pending = None;
        if project_changed {
            *self = Self::default();
        }
    }

    fn slider(
        &self,
        ui: &mut egui::Ui,
        name: &str,
        target: Target,
        channel: Channel,
        control: Control,
    ) -> (i16, Option<Event>) {
        let owns = |change: MixChange| change.target == target && change.control == control;
        let drag = self.gesture.filter(|(change, _)| owns(*change));
        let value = drag
            .map(|(change, _)| change.value)
            .or_else(|| {
                self.pending
                    .filter(|change| owns(*change))
                    .map(|change| change.value)
            })
            .unwrap_or_else(|| channel.get(control));
        let adjustment = controls::Slider {
            control,
            value,
            drag_start: drag.map(|(_, origin)| origin),
            name,
            master: target == Target::Master,
        }
        .show(ui);
        (
            value,
            adjustment.map(|adjustment| Event::Adjust(target, control, adjustment)),
        )
    }
}

impl App {
    fn mixer_selection(&self) -> Option<Target> {
        if self.mixer_ui.master_selected || self.view.project.tracks.is_empty() {
            Some(Target::Master)
        } else {
            self.view.selected.map(Target::Track)
        }
    }

    fn mixer_neighbor(&self, forward: bool) -> Option<Target> {
        self.mixer_neighbor_from(self.mixer_selection()?, forward)
    }

    fn mixer_neighbor_from(&self, target: Target, forward: bool) -> Option<Target> {
        let tracks = &self.view.project.tracks;
        match target {
            Target::Master if !forward => tracks.last().map(|track| Target::Track(track.id)),
            Target::Master => None,
            Target::Track(id) => {
                let index = tracks.iter().position(|track| track.id == id)?;
                if forward {
                    Some(
                        tracks
                            .get(index + 1)
                            .map_or(Target::Master, |track| Target::Track(track.id)),
                    )
                } else {
                    index
                        .checked_sub(1)
                        .map(|index| Target::Track(tracks[index].id))
                }
            }
        }
    }

    pub(super) fn mixer_command_available(&self, command: Command) -> bool {
        match command {
            Command::ShowMixer => true,
            _ if !self.mixer_ui.open => false,
            Command::MixerPrevious | Command::MixerNext => {
                self.mixer_neighbor(command == Command::MixerNext).is_some()
            }
            Command::MixerMute | Command::MixerSolo => {
                matches!(self.mixer_selection(), Some(Target::Track(_)))
            }
            Command::ResetMixerClip => self
                .mixer_selection()
                .is_some_and(|target| self.meter(target).clipped),
            _ => false,
        }
    }

    pub(super) fn dispatch_mixer(&mut self, command: Command) {
        match command {
            Command::ShowMixer => {
                self.cancel_mix_gesture();
                self.mixer_ui.open = !self.mixer_ui.open;
                self.mixer_ui.effect_picker = None;
            }
            Command::MixerPrevious | Command::MixerNext => {
                if let Some(target) = self.mixer_neighbor(command == Command::MixerNext) {
                    self.focus_mixer(target);
                }
            }
            _ => {
                if let Some(target) = self.mixer_selection() {
                    self.run_mixer_command(command, target);
                }
            }
        }
    }

    fn focus_mixer(&mut self, target: Target) {
        self.cancel_mix_gesture();
        self.mixer_ui.focus = Some(target);
        self.mixer_ui.master_selected = target == Target::Master;
        if let Target::Track(id) = target
            && self.view.selected != Some(id)
        {
            self.select_track(id);
        }
    }

    fn meter(&self, target: Target) -> Meter {
        match target {
            Target::Master => self.view.master_meter,
            Target::Track(id) => self.view.track_meters.get(&id).copied().unwrap_or_default(),
        }
    }

    fn make_change(&self, target: Target, control: Control, value: i16) -> MixChange {
        MixChange {
            epoch: self.view.project_epoch,
            target,
            control,
            value,
        }
    }

    fn commit_mixer(&mut self, change: MixChange) {
        let actions = self.actions_in_flight;
        self.send(Action::CommitMix(change));
        if self.actions_in_flight > actions {
            self.mixer_ui.pending = Some(change);
        } else {
            let _ = self.backend.send.send(Action::PreviewMix(None));
        }
        self.mixer_ui.gesture = None;
    }

    fn run_mixer_command(&mut self, command: Command, target: Target) {
        if command == Command::ResetMixerClip {
            self.send(Action::ResetMeter(self.view.project_epoch, target));
            return;
        }
        let control = if command == Command::MixerMute {
            Control::Mute
        } else {
            Control::Solo
        };
        if target.accepts(control)
            && let Ok(channel) = self.view.project.mix(target)
        {
            self.commit_mixer(self.make_change(target, control, 1 - channel.get(control)));
        }
    }

    pub(super) fn cancel_mix_gesture(&mut self) {
        if self.mixer_ui.gesture.take().is_some() {
            let _ = self.backend.send.send(Action::PreviewMix(None));
        }
    }

    pub(super) fn mixer_logic(&mut self, ctx: &egui::Context) {
        if self.mixer_ui.gesture.is_some()
            && (self.confirm.is_some()
                || self.file.is_some()
                || self.busy_since.is_some()
                || ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
                || !ctx.input(|input| input.focused))
        {
            self.cancel_mix_gesture();
        }
    }

    fn mixer_controls_enabled(&self) -> bool {
        self.busy_since.is_none() && self.confirm.is_none() && self.file.is_none()
    }

    pub(super) fn show_mixer(&mut self, ui: &mut egui::Ui) {
        let effects = self
            .view
            .project
            .tracks
            .iter()
            .map(|track| track.inserts.len())
            .chain([self.view.project.master_inserts.len()])
            .max()
            .unwrap_or(0)
            .min(2);
        self.mixer_ui.visible_insert_rows =
            1 + effects + usize::from(!self.view.project.tracks.is_empty());
        let enabled = self.mixer_controls_enabled();
        let selected = self.mixer_selection();
        let mut event = None;
        let selection = if enabled {
            self.mixer_ui.focus.take()
        } else {
            None
        };
        egui::ScrollArea::vertical()
            .id_salt("mixer-vertical")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = MIX_GAP;
                    let bank_width = (ui.available_width() - MIX_CHANNEL - MIX_GAP).max(1.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(bank_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_max_width(bank_width);
                            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                            ui.spacing_mut().scroll.foreground_color = true;
                            let scroll_bar = if self.view.project.tracks.len() as f32
                                * (MIX_CHANNEL + MIX_GAP)
                                > bank_width
                            {
                                egui::scroll_area::ScrollBarVisibility::AlwaysVisible
                            } else {
                                egui::scroll_area::ScrollBarVisibility::AlwaysHidden
                            };
                            egui::ScrollArea::horizontal()
                                .id_salt("mixer-bank")
                                .auto_shrink([false, true])
                                .max_width(bank_width)
                                .scroll_bar_visibility(scroll_bar)
                                .show(ui, |ui| {
                                    ui.horizontal_top(|ui| {
                                        ui.spacing_mut().item_spacing.x = MIX_GAP;
                                        for track in &self.view.project.tracks {
                                            let target = Target::Track(track.id);
                                            let (response, result) = self.mixer_strip(
                                                ui,
                                                target,
                                                &track.name,
                                                track.mix,
                                                selected == Some(target),
                                                selection == Some(target),
                                            );
                                            if selection == Some(target) {
                                                response.scroll_to_me(Some(egui::Align::Center));
                                            }
                                            event = result.or(event.take());
                                        }
                                    });
                                });
                        },
                    );
                    let target = Target::Master;
                    let channel = Channel {
                        level: self.view.project.master,
                        ..Default::default()
                    };
                    let (_, result) = self.mixer_strip(
                        ui,
                        target,
                        "Master",
                        channel,
                        selected == Some(target),
                        selection == Some(target),
                    );
                    event = result.or(event.take());
                });
                if !self.status.is_empty() {
                    controls::small_label(ui, &self.status);
                }
            });
        match event {
            Some(Event::FocusInsert(target, id)) if enabled => {
                self.mixer_ui.insert_focus = Some((target, id));
            }
            Some(Event::FocusInstrument(id)) if enabled => {
                self.mixer_ui.instrument_focus = Some(id);
            }
            Some(Event::Instrument(command, id)) if enabled => {
                self.mixer_ui.instrument_focus = Some(id);
                self.run_instrument_command(command, id, ui.ctx());
            }
            Some(Event::Insert(command, target, id)) if enabled => {
                self.run_insert_command(command, target, id, ui.ctx());
            }
            Some(Event::DropInsert(target, id, before)) if enabled => {
                self.send_insert(target, InsertAction::Move { id, before });
            }
            Some(Event::Focus(target)) if enabled => self.focus_mixer(target),
            Some(Event::Command(command, target)) if enabled => {
                self.run_mixer_command(command, target)
            }
            Some(Event::Adjust(target, control, controls::Adjustment::Commit(value)))
                if enabled =>
            {
                self.commit_mixer(self.make_change(target, control, value));
            }
            Some(Event::Adjust(target, control, controls::Adjustment::Drag { origin, value }))
                if enabled =>
            {
                let change = self.make_change(target, control, value);
                if self.mixer_ui.gesture.is_none_or(|(previous, _)| {
                    previous.value != value
                        || previous.control != control
                        || previous.target != target
                }) {
                    let _ = self.backend.send.send(Action::PreviewMix(Some(change)));
                }
                self.mixer_ui.gesture = Some((change, origin));
                ui.ctx().request_repaint();
            }
            _ => {
                if !ui.input(|input| input.pointer.primary_down()) {
                    self.cancel_mix_gesture();
                }
            }
        }
        self.show_effect_picker(ui.ctx());
    }

    fn mixer_strip(
        &self,
        ui: &mut egui::Ui,
        target: Target,
        name: &str,
        channel: Channel,
        selected: bool,
        focus: bool,
    ) -> (egui::Response, Option<Event>) {
        let mut event = None;
        let enabled = self.mixer_controls_enabled();
        let master = target == Target::Master;
        let meter = self.meter(target);
        let shortcut = |command| {
            self.bindings
                .iter()
                .find(|binding| binding.command == command)
                .and_then(|binding| binding.shortcut)
                .map(|shortcut| ui.ctx().format_shortcut(&shortcut))
                .unwrap_or_default()
        };
        let shortcuts = [
            Command::MixerMute,
            Command::MixerSolo,
            Command::ResetMixerClip,
        ]
        .map(shortcut);
        let frame = ui
            .push_id(target, |ui| {
                controls::strip(ui, selected, |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        let knob = |ui: &mut egui::Ui, control: Control| {
                            ui.vertical_centered(|ui| {
                                controls::small_label(ui, control.label());
                                let (value, adjustment) =
                                    self.mixer_ui.slider(ui, name, target, channel, control);
                                controls::small_label(ui, &controls::text_value(control, value));
                                adjustment
                            })
                            .inner
                        };
                        if master {
                            ui.scope_builder(egui::UiBuilder::new().invisible(), |ui| {
                                knob(ui, Control::Gain)
                            });
                        } else {
                            event = knob(ui, Control::Gain).or(event.take());
                        }
                        ui.add_space(MIX_GROUP - MIX_ROW);
                        event = self.mixer_inserts(ui, target, name).or(event.take());
                        ui.add_space(MIX_GROUP - MIX_ROW);
                        if master {
                            event = knob(ui, Control::Pan).or(event.take());
                        } else {
                            ui.columns(2, |columns| {
                                event = knob(&mut columns[0], Control::Pan).or(event.take());
                                event = knob(&mut columns[1], Control::Width).or(event.take());
                            });
                        }
                        ui.add_space(MIX_GROUP - MIX_ROW);
                        let mut level = channel.level.fader;
                        ui.horizontal_top(|ui| {
                            let (value, adjustment) =
                                self.mixer_ui
                                    .slider(ui, name, target, channel, Control::Fader);
                            level = value;
                            event = adjustment.or(event.take());
                            controls::scale(ui);
                            if controls::strip_meter(ui, name, meter, &shortcuts[2])
                                && meter.clipped
                            {
                                event = Some(Event::Command(Command::ResetMixerClip, target));
                            }
                        });
                        ui.vertical_centered(|ui| {
                            controls::small_label(ui, &controls::text_value(Control::Fader, level))
                        });
                        ui.add_space(MIX_GROUP - MIX_ROW);
                        if master {
                            ui.allocate_space(egui::vec2(0.0, ui.spacing().interact_size.y));
                        } else {
                            ui.horizontal(|ui| {
                                for (index, (control, label, command, active)) in [
                                    (Control::Mute, "M", Command::MixerMute, channel.mute),
                                    (Control::Solo, "S", Command::MixerSolo, channel.solo),
                                ]
                                .into_iter()
                                .enumerate()
                                {
                                    let mut hint = format!("{} {name}", control.label());
                                    if !shortcuts[index].is_empty() {
                                        hint.push_str(&format!(" ({})", shortcuts[index]));
                                    }
                                    if controls::lamp(
                                        ui,
                                        label,
                                        &hint,
                                        control == Control::Solo,
                                        active,
                                        enabled,
                                    ) {
                                        event = Some(Event::Command(command, target));
                                    }
                                }
                            });
                        }
                        ui.add_space(MIX_GROUP - MIX_ROW);
                        ui.vertical_centered(|ui| {
                            controls::small_label(ui, if master { "Output" } else { "To Master" })
                        });
                        let response = controls::footer(ui, name, master, name);
                        if focus {
                            response.request_focus();
                        }
                        if response.clicked() {
                            event = Some(Event::Focus(target));
                        }
                        if response.has_focus() {
                            let move_forward = ui.input_mut(|input| {
                                if input.consume_key(Modifiers::NONE, Key::ArrowRight) {
                                    Some(true)
                                } else if input.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                                    Some(false)
                                } else {
                                    None
                                }
                            });
                            if let Some(next) = move_forward
                                .and_then(|forward| self.mixer_neighbor_from(target, forward))
                            {
                                event = Some(Event::Focus(next));
                            }
                        }
                    });
                })
            })
            .inner;
        (frame, event)
    }

    pub(super) fn insert_command_available(&self, command: Command) -> bool {
        self.mixer_ui.open
            && self.mixer_ui.effect_picker.is_none()
            && if command == Command::InsertAdd {
                self.mixer_selection().is_some()
            } else {
                self.mixer_ui
                    .insert_focus
                    .is_some_and(|(target, id)| self.insert_command_enabled(command, target, id))
            }
    }

    pub(super) fn dispatch_insert(&mut self, command: Command, ctx: &egui::Context) {
        if command == Command::InsertAdd {
            if let Some(target) = self.mixer_selection() {
                self.run_insert_command(command, target, None, ctx);
            }
        } else if let Some((target, id)) = self.mixer_ui.insert_focus {
            self.run_insert_command(command, target, Some(id), ctx);
        }
    }

    fn instrument_target(&self) -> Option<u64> {
        self.mixer_ui.instrument_focus.or_else(|| {
            self.mixer_selection().and_then(|target| match target {
                Target::Track(id) => Some(id),
                Target::Master => None,
            })
        })
    }

    pub(super) fn instrument_command_available(&self, command: Command) -> bool {
        if !self.mixer_ui.open || self.mixer_ui.effect_picker.is_some() {
            return false;
        }
        let Some(id) = self.instrument_target() else {
            return false;
        };
        let Some(track) = self.view.project.tracks.iter().find(|track| track.id == id) else {
            return false;
        };
        match command {
            Command::InstrumentBypass => track.plugin.is_some() && !track.instrument_offline,
            Command::InstrumentOffline => track.plugin.is_some(),
            _ => false,
        }
    }

    pub(super) fn dispatch_instrument(&mut self, command: Command, ctx: &egui::Context) {
        if let Some(id) = self.instrument_target() {
            self.run_instrument_command(command, id, ctx);
        }
    }

    fn run_instrument_command(&mut self, command: Command, id: u64, ctx: &egui::Context) {
        let Some(track) = self
            .view
            .project
            .tracks
            .iter()
            .find(|track| track.id == id)
            .cloned()
        else {
            return;
        };
        let has_plugin = track.plugin.is_some();
        let offline = track.instrument_offline;
        match command {
            Command::Instrument if !offline => {
                self.select_track(id);
                self.picker_open = true;
                if self.catalog.plugins.is_empty() {
                    self.scan(ctx.clone());
                }
            }
            Command::Editor if has_plugin && !offline => {
                self.send(Action::Editor(id, true));
            }
            Command::InstrumentBypass if has_plugin && !offline => {
                self.send(Action::InstrumentBypass {
                    epoch: self.view.project_epoch,
                    id,
                    bypassed: !track.instrument_bypassed,
                });
            }
            Command::InstrumentOffline if has_plugin => {
                self.send(Action::InstrumentOffline {
                    epoch: self.view.project_epoch,
                    id,
                    offline: !offline,
                });
            }
            _ => {}
        }
    }

    fn send_insert(&mut self, target: Target, action: InsertAction) {
        self.send(Action::Insert(self.view.project_epoch, target, action));
    }

    fn insert_command_enabled(&self, command: Command, target: Target, id: u64) -> bool {
        let Some(chain) = self.view.project.inserts(target) else {
            return false;
        };
        let Some(index) = chain.iter().position(|slot| slot.id == id) else {
            return false;
        };
        match command {
            Command::InsertOpen | Command::InsertBypass => self
                .view
                .inserts
                .get(&id)
                .is_none_or(|view| !view.missing && view.error.is_empty()),
            Command::InsertMoveUp => index > 0,
            Command::InsertMoveDown => index + 1 < chain.len(),
            Command::InsertAfter | Command::InsertRemove => true,
            _ => false,
        }
    }

    fn run_insert_command(
        &mut self,
        command: Command,
        target: Target,
        id: Option<u64>,
        ctx: &egui::Context,
    ) {
        let Some(chain) = self.view.project.inserts(target) else {
            return;
        };
        if command == Command::InsertAdd {
            self.mixer_ui.effect_picker = Some((target, id));
            self.mixer_ui.effect_search.clear();
            if self.catalog.effects.is_empty() {
                self.scan(ctx.clone());
            }
            return;
        }
        let Some(id) = id.filter(|id| self.insert_command_enabled(command, target, *id)) else {
            return;
        };
        self.mixer_ui.insert_focus = Some((target, id));
        let index = chain
            .iter()
            .position(|slot| slot.id == id)
            .expect("available insert");
        let action = match command {
            Command::InsertAfter => {
                let before = chain.get(index + 1).map(|slot| slot.id);
                self.run_insert_command(Command::InsertAdd, target, before, ctx);
                return;
            }
            Command::InsertOpen => InsertAction::Editor(id),
            Command::InsertBypass => InsertAction::Bypass(id),
            Command::InsertRemove => InsertAction::Remove(id),
            Command::InsertMoveUp => InsertAction::Move {
                id,
                before: Some(chain[index - 1].id),
            },
            Command::InsertMoveDown => InsertAction::Move {
                id,
                before: chain.get(index + 2).map(|slot| slot.id),
            },
            _ => return,
        };
        self.send_insert(target, action);
    }

    fn show_effect_picker(&mut self, ctx: &egui::Context) {
        let Some((target, before)) = self.mixer_ui.effect_picker else {
            return;
        };
        let entries = self
            .catalog
            .effects
            .iter()
            .enumerate()
            .map(|(index, plugin)| {
                let enabled = self.busy_since.is_none()
                    && !self
                        .view
                        .quarantine
                        .iter()
                        .any(|entry| crate::quarantine::same(&entry.plugin, plugin));
                (
                    index,
                    format!("{} · {}", plugin.name, plugin.format),
                    enabled,
                )
            });
        let (selected, scan, cancel) = inserts::picker(
            ctx,
            &mut self.mixer_ui.effect_search,
            self.scan_job.is_some(),
            entries,
        );
        if cancel || selected.is_some() {
            self.mixer_ui.effect_picker = None;
        }
        if scan {
            self.scan(ctx.clone());
        }
        if let Some(plugin) = selected
            .and_then(|index| self.catalog.effects.get(index))
            .cloned()
        {
            self.send_insert(target, InsertAction::Add { before, plugin });
        }
    }

    fn mixer_inserts(&self, ui: &mut egui::Ui, target: Target, name: &str) -> Option<Event> {
        let chain = self.view.project.inserts(target)?;
        let track = match target {
            Target::Track(id) => self.view.project.tracks.iter().find(|track| track.id == id),
            Target::Master => None,
        };
        let mut event = None;
        controls::small_label(ui, "Inserts");
        let height = inserts::SLOT_HEIGHT + inserts::GAP_HEIGHT;
        let rows = self.mixer_ui.visible_insert_rows.max(1);
        let reserved_vi = track.is_some() || !self.view.project.tracks.is_empty();
        let effects_rows = if reserved_vi {
            rows.saturating_sub(1).max(1)
        } else {
            rows
        };

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), rows as f32 * height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_min_size(egui::vec2(ui.available_width(), rows as f32 * height));
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.spacing_mut().button_padding.y = 0.0;
                ui.spacing_mut().interact_size.y = inserts::SLOT_HEIGHT;
                if let Some(track) = track {
                    self.instrument_slot(ui, track, name, &mut event);
                }
                egui::ScrollArea::vertical()
                    .id_salt("insert-chain")
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .max_height(effects_rows as f32 * height)
                    .auto_shrink([false, false])
                    .show_rows(ui, height, chain.len() + 1, |ui, rows| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for index in rows {
                            let Some(slot) = chain.get(index) else {
                                ui.add_space(inserts::GAP_HEIGHT);
                                let append = inserts::append(ui, name);
                                if append.clicked() {
                                    event = Some(Event::Insert(Command::InsertAdd, target, None));
                                }
                                if let Some(source) = append
                                    .dnd_release_payload::<DragInsert>()
                                    .filter(|s| s.0 == target)
                                {
                                    event = Some(Event::DropInsert(target, source.1, None));
                                }
                                continue;
                            };
                            ui.push_id(slot.id, |ui| {
                                if index > 0 {
                                    let gap = inserts::gap(
                                        ui,
                                        &format!(
                                            "{name}, insert effect before {}",
                                            slot.plugin.name
                                        ),
                                        self.mixer_ui.gallery_insert_focus == Some(slot.id),
                                    );
                                    if gap.clicked() {
                                        event = Some(Event::Insert(
                                            Command::InsertAdd,
                                            target,
                                            Some(slot.id),
                                        ));
                                    }
                                    if let Some(source) = gap
                                        .dnd_release_payload::<DragInsert>()
                                        .filter(|s| s.0 == target)
                                    {
                                        event = Some(Event::DropInsert(
                                            target,
                                            source.1,
                                            Some(slot.id),
                                        ));
                                    }
                                } else {
                                    ui.add_space(inserts::GAP_HEIGHT);
                                }
                                let status = self.view.inserts.get(&slot.id);
                                let (state, description) =
                                    if status.is_some_and(|status| status.missing) {
                                        (inserts::State::Missing, "Missing")
                                    } else if let Some(error) = status
                                        .map(|status| status.error.as_str())
                                        .filter(|s| !s.is_empty())
                                    {
                                        (inserts::State::Failed, error)
                                    } else if slot.bypassed {
                                        (inserts::State::Bypassed, "Bypassed")
                                    } else {
                                        (inserts::State::Active, "Open original editor")
                                    };
                                let (response, activate, power) = inserts::slot(
                                    ui,
                                    &slot.plugin.name,
                                    name,
                                    "Effect",
                                    state,
                                    description,
                                );
                                response.dnd_set_drag_payload(DragInsert(target, slot.id));
                                if response.has_focus()
                                    && self.mixer_ui.insert_focus != Some((target, slot.id))
                                    && event.is_none()
                                {
                                    event = Some(Event::FocusInsert(target, slot.id));
                                }
                                if activate {
                                    event = Some(Event::Insert(
                                        Command::InsertOpen,
                                        target,
                                        Some(slot.id),
                                    ));
                                }
                                if power {
                                    event = Some(Event::Insert(
                                        Command::InsertBypass,
                                        target,
                                        Some(slot.id),
                                    ));
                                }
                                response.context_menu(|ui| {
                                    for (command, label) in [
                                        (Command::InsertAfter, "Insert Effect After…"),
                                        (Command::InsertMoveUp, "Move Up"),
                                        (Command::InsertMoveDown, "Move Down"),
                                        (Command::InsertRemove, "Remove"),
                                    ] {
                                        if inserts::menu_item(
                                            ui,
                                            label,
                                            self.insert_command_enabled(command, target, slot.id),
                                        ) {
                                            event =
                                                Some(Event::Insert(command, target, Some(slot.id)));
                                        }
                                    }
                                });
                            });
                        }
                    });
                if track.is_none() && reserved_vi {
                    ui.add_space(height);
                }
            },
        );
        event
    }

    fn instrument_slot(
        &self,
        ui: &mut egui::Ui,
        track: &clef_model::project::Track,
        name: &str,
        event: &mut Option<Event>,
    ) {
        let plugin = track.plugin.as_ref();
        let offline = track.instrument_offline;
        let bypassed = track.instrument_bypassed;
        let quarantine = plugin.is_some_and(|plugin| {
            self.view
                .quarantine
                .iter()
                .any(|entry| crate::quarantine::same(&entry.plugin, plugin))
        });
        let missing = plugin.is_some_and(|plugin| !plugin.path.exists());
        let track_view = self.view.tracks.iter().find(|view| view.id == track.id);
        let failed = track_view.is_some_and(|view| view.status == "Unavailable");
        let loading = track_view.is_some_and(|view| view.loading);
        let (state, description, ready) = if plugin.is_none() {
            (inserts::State::Empty, "Choose an instrument", false)
        } else if offline {
            (inserts::State::Offline, "Offline", false)
        } else if missing {
            (inserts::State::Missing, "Missing", false)
        } else if quarantine {
            (inserts::State::Failed, "Quarantined", false)
        } else if failed {
            (
                inserts::State::Failed,
                track_view.map_or("Instrument unavailable", |view| view.status.as_str()),
                false,
            )
        } else if bypassed {
            (inserts::State::Bypassed, "Bypassed", true)
        } else if loading {
            (inserts::State::Active, "Loading instrument…", true)
        } else {
            (inserts::State::Active, "Open instrument editor", true)
        };
        let display = plugin.map_or("No instrument", |plugin| plugin.name.as_str());
        ui.push_id("instrument-slot", |ui| {
            ui.add_space(inserts::GAP_HEIGHT);
            let (response, activate, power) =
                inserts::slot(ui, display, name, "Plugin", state, description);
            if response.has_focus()
                && self.mixer_ui.instrument_focus != Some(track.id)
                && event.is_none()
            {
                *event = Some(Event::FocusInstrument(track.id));
            }
            if activate {
                *event = Some(Event::Instrument(
                    if plugin.is_some() && !offline && !missing && !quarantine && !failed {
                        Command::Editor
                    } else {
                        Command::Instrument
                    },
                    track.id,
                ));
            }
            if power {
                *event = Some(Event::Instrument(Command::InstrumentBypass, track.id));
            }
            response.context_menu(|ui| {
                if !offline {
                    if plugin.is_some() {
                        if inserts::menu_item(ui, "Open Instrument Editor", ready) {
                            *event = Some(Event::Instrument(Command::Editor, track.id));
                        }
                        if inserts::menu_item(ui, "Replace Instrument…", true) {
                            *event = Some(Event::Instrument(Command::Instrument, track.id));
                        }
                    } else if inserts::menu_item(ui, "Choose Instrument…", true) {
                        *event = Some(Event::Instrument(Command::Instrument, track.id));
                    }
                    if plugin.is_some() {
                        let label = if bypassed {
                            "Enable Instrument"
                        } else {
                            "Bypass Instrument"
                        };
                        if inserts::menu_item(ui, label, true) {
                            *event = Some(Event::Instrument(Command::InstrumentBypass, track.id));
                        }
                    }
                }
                if plugin.is_some() {
                    let label = if offline {
                        "Bring Instrument Online"
                    } else {
                        "Take Instrument Offline"
                    };
                    if inserts::menu_item(ui, label, true) {
                        *event = Some(Event::Instrument(Command::InstrumentOffline, track.id));
                    }
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clef_model::{
        Plugin,
        project::{Insert, Project, Track},
    };
    use std::path::PathBuf;

    fn insert(id: u64, name: &str) -> Insert {
        Insert {
            id,
            plugin: Plugin {
                path: format!("/Fixtures/{name}.vst3").into(),
                id: format!("fixture-{id}"),
                name: name.into(),
                format: "VST3".into(),
            },
            bypassed: false,
            state: Default::default(),
        }
    }

    fn app_with_two_inserts() -> App {
        let ctx = egui::Context::default();
        let mut app = App::new(ctx, PathBuf::new());
        let mut project = Project::default();
        project.tracks.push(Track {
            id: 1,
            name: "Strings".into(),
            plugin: None,
            instrument_bypassed: false,
            instrument_offline: false,
            record_mode: Default::default(),
            take_lanes: Default::default(),
            mix: Default::default(),
            inserts: vec![insert(2, "Tape EQ"), insert(3, "Stereo Delay")],
            state: Vec::new(),
        });
        project.next_id = 4;
        app.view.project = project;
        app.view.project_open = true;
        app.view.selected = Some(1);
        app.mixer_ui.open = true;
        app.catalog.effects = vec![insert(4, "Plate Reverb").plugin];
        app
    }

    #[test]
    fn insert_after_command_opens_the_picker_at_the_following_slot() {
        let ctx = egui::Context::default();
        let mut app = app_with_two_inserts();
        let target = Target::Track(1);
        app.mixer_ui.insert_focus = Some((target, 2));
        app.dispatch_insert(Command::InsertAfter, &ctx);
        assert_eq!(app.mixer_ui.effect_picker, Some((target, Some(3))));
    }

    #[test]
    fn instrument_commands_need_an_online_instrument() {
        let mut app = app_with_two_inserts();
        let id = app.view.project.tracks[0].id;
        app.mixer_ui.instrument_focus = Some(id);
        assert!(!app.instrument_command_available(Command::InstrumentBypass));
        assert!(!app.instrument_command_available(Command::InstrumentOffline));
        app.view.project.tracks[0].plugin = Some(insert(4, "Instrument").plugin);
        assert!(app.instrument_command_available(Command::InstrumentBypass));
        assert!(app.instrument_command_available(Command::InstrumentOffline));
        app.view.project.tracks[0].instrument_offline = true;
        assert!(!app.instrument_command_available(Command::InstrumentBypass));
        assert!(app.instrument_command_available(Command::InstrumentOffline));
    }

    #[test]
    fn insert_add_command_opens_the_picker_at_the_end_for_the_selected_channel() {
        let ctx = egui::Context::default();
        let mut app = app_with_two_inserts();
        app.dispatch_insert(Command::InsertAdd, &ctx);
        assert_eq!(app.mixer_ui.effect_picker, Some((Target::Track(1), None)));
    }
}
