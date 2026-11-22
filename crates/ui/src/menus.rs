//! The project window's menu bar (T18 Phase 2): every entry is a registry command with its live
//! name, availability and shortcut; mode groups are captioned sections with ticks.
use crate::{
    App,
    commands::Command,
    primitives::{menu, menu_bar, text_button::TextButton},
};
use clef_model::{midi::Mode, project::TrackMode, timeline::StopMode};
use eframe::egui::Ui;

const TITLES: [&str; 6] = ["File", "Edit", "Track", "Transport", "View", "Window"];

impl App {
    /// One command row; `ticked` makes it a checkable row.
    pub(crate) fn menu_command(&self, ui: &mut Ui, command: Command, ticked: Option<bool>) -> bool {
        let label = self.command_label(command);
        let shortcut = self.keymap.text(ui.ctx(), command);
        let mut item = menu::Item::new(&label)
            .shortcut(&shortcut)
            .enabled(self.enabled(command))
            .destructive(command.destructive());
        if let Some(on) = ticked {
            item = item.ticked(on);
        }
        item.show(ui)
    }

    fn menu_commands(&self, ui: &mut Ui, commands: &[Command]) -> Option<Command> {
        commands.iter().copied().fold(None, |picked, command| {
            picked.or(self.menu_command(ui, command, None).then_some(command))
        })
    }

    fn menu_choices(&self, ui: &mut Ui, choices: &[(Command, bool)]) -> Option<Command> {
        choices.iter().fold(None, |picked, &(command, on)| {
            picked.or(self.menu_command(ui, command, Some(on)).then_some(command))
        })
    }

    pub(crate) fn menu_bar(&self, ui: &mut Ui) -> Option<Command> {
        use Command::*;
        let timeline = &self.view.project.timeline;
        let track_mode = self.view.track().map(|track| track.record_mode);
        menu_bar::menu_bar(
            ui,
            menu_bar::main_id(),
            &TITLES,
            |index, ui| match index {
                0 => {
                    let picked = self.menu_commands(ui, &[New, Open]);
                    menu::separator(ui);
                    let saved = self.menu_commands(ui, &[Save, SaveAs]);
                    menu::separator(ui);
                    picked.or(saved).or(self.menu_commands(ui, &[Close]))
                }
                1 => {
                    let picked = self.menu_commands(ui, &[Undo, Redo, EditHistory]);
                    menu::separator(ui);
                    picked.or(self.menu_commands(
                        ui,
                        &[SelectAllClips, DuplicateClips, SplitClips, DeleteClips],
                    ))
                }
                2 => {
                    let mut picked = self.menu_commands(
                        ui,
                        &[
                            NewTracks,
                            AddTrack,
                            AddBus,
                            Rename,
                            MoveUp,
                            MoveDown,
                            RemoveTrack,
                        ],
                    );
                    menu::separator(ui);
                    picked = picked.or(self.menu_commands(
                        ui,
                        &[
                            Instrument,
                            Editor,
                            CloseEditor,
                            Restart,
                            InstrumentBypass,
                            InstrumentOffline,
                        ],
                    ));
                    menu::separator(ui);
                    picked = picked.or(self.menu_commands(ui, &[ArmTrack, ShowTakeLanes]));
                    menu::caption(ui, "Track Record Mode");
                    picked.or(self.menu_choices(
                        ui,
                        &[
                            (RecordTrackGlobal, track_mode == Some(TrackMode::Global)),
                            (RecordTrackMerge, track_mode == Some(TrackMode::Merge)),
                            (RecordTrackReplace, track_mode == Some(TrackMode::Replace)),
                            (RecordTrackNewTake, track_mode == Some(TrackMode::NewTake)),
                        ],
                    ))
                }
                3 => {
                    let mut picked = self.menu_commands(ui, &[PlayStop, ReturnToStart, Record]);
                    menu::separator(ui);
                    picked = picked.or(self.menu_choices(
                        ui,
                        &[
                            (ToggleLoop, timeline.looping),
                            (ToggleClick, timeline.click),
                            (ToggleCountIn, timeline.count_in),
                        ],
                    ));
                    let mode = self.view.project.record_mode;
                    menu::caption(ui, "Record Mode");
                    picked = picked.or(self.menu_choices(
                        ui,
                        &[
                            (RecordMerge, mode == Mode::Merge),
                            (RecordReplace, mode == Mode::Replace),
                            (RecordNewTake, mode == Mode::NewTake),
                        ],
                    ));
                    menu::caption(ui, "Stop Mode");
                    picked = picked.or(self.menu_choices(
                        ui,
                        &[
                            (
                                KeepStopPosition,
                                timeline.stop_mode == StopMode::KeepPosition,
                            ),
                            (
                                ReturnStopPosition,
                                timeline.stop_mode == StopMode::ReturnToPlayStart,
                            ),
                        ],
                    ));
                    menu::separator(ui);
                    picked.or(self.menu_commands(ui, &[TempoMap, TransportSettings, ProjectFormat]))
                }
                4 => {
                    let picked = self.menu_commands(ui, &[ShowMixer, ShowEditor, ToggleLeftDrawer]);
                    menu::separator(ui);
                    picked.or(self.menu_commands(
                        ui,
                        &[
                            FitArrangementWidth,
                            FitArrangementHeight,
                            FitRollWidth,
                            FitRollHeight,
                        ],
                    ))
                }
                _ => {
                    let picked =
                        self.menu_commands(ui, &[Commands, Palette, Settings, TrackDiagnostics]);
                    menu::separator(ui);
                    let picked = picked.or(self.menu_commands(ui, &[Quarantine, Rescan, Refresh]));
                    // Debug builds only, and not a registry command: a developer view that
                    // release builds do not ship.
                    if cfg!(debug_assertions) {
                        menu::separator(ui);
                        if menu::Item::new("Primitive Gallery (Debug)").show(ui) {
                            self.primitive_gallery_open.set(true);
                        }
                    }
                    picked
                }
            },
            |ui| {
                // T41: Settings lives in the Window menu only; Commands… stays here.
                let palette = self.keymap.tooltip(ui.ctx(), Palette);
                let commands = TextButton::new("Commands…")
                    .tooltip(&palette)
                    .enabled(self.command_available(Palette, false))
                    .show(ui)
                    .clicked();
                (commands && self.enabled(Palette)).then_some(Palette)
            },
        )
    }
}
