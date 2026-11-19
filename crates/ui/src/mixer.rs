use super::{App, Command, Key, Modifiers, egui, primitives::mixer as controls, theme::*};
use clef_model::mixer::{Channel, Control, Target};
use clef_session::{Action, Meter, MixChange};

#[derive(Default)]
pub(super) struct MixerUi {
    pub open: bool,
    master_selected: bool,
    focus: Option<Target>,
    gesture: Option<(MixChange, controls::DragOrigin)>,
    pending: Option<MixChange>,
}

enum Event {
    Focus(Target),
    Adjust(Target, Control, controls::Adjustment),
    Command(Command, Target),
}

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
}
