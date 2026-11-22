//! The command registry (T18 Phase 2): every Clef command's stable id, title, default
//! shortcut and context, the live key map, and the shortcut matching rules. Menus, tooltips,
//! the command palette and the Commands window read names and shortcuts only from here.
use eframe::egui::{self, InputState, Key, KeyboardShortcut, Modifiers};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub(crate) enum Command {
    New,
    Open,
    Save,
    Undo,
    Redo,
    EditHistory,
    ShowMixer,
    ShowEditor,
    ToggleLeftDrawer,
    TogglePanelFull,
    TogglePanelDetach,
    WiderTrackColumn,
    NarrowerTrackColumn,
    NewTracks,
    ProjectFormat,
    MixerPrevious,
    MixerNext,
    MixerMute,
    MixerSolo,
    ResetMixerClip,
    InsertAdd,
    InsertAfter,
    InsertOpen,
    InsertBypass,
    InsertMoveUp,
    InsertMoveDown,
    InsertRemove,
    InstrumentBypass,
    InstrumentOffline,
    CreateClip,
    SelectAllClips,
    DuplicateClips,
    DeleteClips,
    SplitClips,
    MoveClipsLeft,
    MoveClipsRight,
    MoveClipsUp,
    MoveClipsDown,
    ResizeClipShorter,
    ResizeClipLonger,
    LoopClipShorter,
    LoopClipLonger,
    SaveAs,
    Close,
    OpenRecent,
    LocateRecent,
    RemoveRecent,
    ShowRecovery,
    RecoverProject,
    DiscardRecovery,
    RecoveryLater,
    Settings,
    TrackDiagnostics,
    Commands,
    Palette,
    AddTrack,
    AddBus,
    Instrument,
    Editor,
    CloseEditor,
    Restart,
    Rescan,
    Refresh,
    Rename,
    ApplySettings,
    RemoveTrack,
    MoveUp,
    MoveDown,
    PreviousTrack,
    NextTrack,
    Quarantine,
    Reactivate,
    Play,
    Record,
    ArmTrack,
    RecordMerge,
    RecordReplace,
    RecordNewTake,
    RecordTrackGlobal,
    RecordTrackMerge,
    RecordTrackReplace,
    RecordTrackNewTake,
    ShowTakeLanes,
    UseWholeTake,
    DeleteTake,
    FlattenComp,
    TakeToNewTrack,
    RenameTake,
    EditTake,
    Stop,
    PlayStop,
    ReturnToStart,
    ToggleLoop,
    ToggleClick,
    ToggleCountIn,
    TempoMap,
    FitArrangementWidth,
    FitArrangementHeight,
    FitRollWidth,
    FitRollHeight,
    TransportSettings,
    KeepStopPosition,
    ReturnStopPosition,
    RollSelectAll,
    RollDelete,
    RollCopy,
    RollPaste,
    RollDuplicate,
    RollEnter,
    RollEarlier,
    RollLater,
    RollUp,
    RollDown,
    RollShorten,
    RollLengthen,
}

/// Where a command's shortcut is live. Two bindings conflict only when their contexts can be
/// active together: Global overlaps every context, the others overlap only themselves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Context {
    Global,
    /// The arrangement has keyboard focus.
    Arrangement,
    /// A piano-roll lane has keyboard focus; the roll consumes these keys itself.
    PianoRoll,
    /// The mixer is shown.
    Mixer,
}

impl Context {
    pub(crate) fn overlaps(self, other: Self) -> bool {
        self == other || self == Self::Global || other == Self::Global
    }
}

pub(crate) struct Spec {
    pub command: Command,
    /// Stable identifier used in the saved shortcut preferences; never reuse or rename.
    pub id: &'static str,
    pub title: &'static str,
    pub default: Option<KeyboardShortcut>,
    pub context: Context,
}

const fn key(modifiers: Modifiers, key: Key) -> Option<KeyboardShortcut> {
    Some(KeyboardShortcut::new(modifiers, key))
}
const fn cmd(k: Key) -> Option<KeyboardShortcut> {
    key(Modifiers::COMMAND, k)
}
const fn plain(k: Key) -> Option<KeyboardShortcut> {
    key(Modifiers::NONE, k)
}
const CMD_SHIFT: Modifiers = Modifiers {
    alt: false,
    ctrl: false,
    shift: true,
    mac_cmd: false,
    command: true,
};
const ALT_SHIFT: Modifiers = Modifiers {
    alt: true,
    ctrl: false,
    shift: true,
    mac_cmd: false,
    command: false,
};
const CMD_ALT: Modifiers = Modifiers {
    alt: true,
    ctrl: false,
    shift: false,
    mac_cmd: false,
    command: true,
};

macro_rules! specs {
    ($($command:ident, $id:literal, $title:literal, $default:expr, $context:ident;)*) => {
        pub(crate) static SPECS: &[Spec] = &[$(Spec {
            command: Command::$command,
            id: $id,
            title: $title,
            default: $default,
            context: Context::$context,
        }),*];
    };
}

specs! {
    New, "file.new", "New Project", cmd(Key::N), Global;
    Open, "file.open", "Open Project…", cmd(Key::O), Global;
    Save, "file.save", "Save", cmd(Key::S), Global;
    SaveAs, "file.save-as", "Save As…", None, Global;
    Close, "file.quit", "Close Clef", cmd(Key::Q), Global;
    Redo, "edit.redo", "Redo", key(CMD_SHIFT, Key::Z), Global;
    Undo, "edit.undo", "Undo", cmd(Key::Z), Global;
    EditHistory, "edit.history", "Edit History…", None, Global;
    ShowMixer, "view.mixer", "Show Mixer", key(CMD_SHIFT, Key::M), Global;
    ShowEditor, "view.editor", "Show Editor", key(CMD_SHIFT, Key::E), Global;
    ToggleLeftDrawer, "view.left-drawer", "Show / Hide Left Drawer", None, Global;
    TogglePanelFull, "view.panel-full", "Lower Panel: Half / Full Height", None, Global;
    TogglePanelDetach, "view.panel-detach", "Lower Panel: Detach / Attach", None, Global;
    WiderTrackColumn, "view.track-column-wider", "Wider Track Column", None, Global;
    NarrowerTrackColumn, "view.track-column-narrower", "Narrower Track Column", None, Global;
    NewTracks, "track.new", "New Tracks…", None, Global;
    ProjectFormat, "transport.project-format", "Project Format…", None, Global;
    MixerPrevious, "mixer.previous", "Mixer: Previous Channel", None, Mixer;
    MixerNext, "mixer.next", "Mixer: Next Channel", None, Mixer;
    MixerMute, "mixer.mute", "Mixer: Toggle Mute", None, Mixer;
    MixerSolo, "mixer.solo", "Mixer: Toggle Solo", None, Mixer;
    ResetMixerClip, "mixer.clear-clip", "Mixer: Clear Clip Indicator", None, Mixer;
    InsertAdd, "insert.add", "Add Audio Effect…", None, Mixer;
    InsertAfter, "insert.after", "Insert Effect After…", None, Mixer;
    InsertOpen, "insert.open", "Open Effect Editor", None, Mixer;
    InsertBypass, "insert.bypass", "Toggle Effect Bypass", None, Mixer;
    InsertMoveUp, "insert.up", "Move Effect Up", None, Mixer;
    InsertMoveDown, "insert.down", "Move Effect Down", None, Mixer;
    InsertRemove, "insert.remove", "Remove Effect", None, Mixer;
    InstrumentBypass, "instrument.bypass", "Toggle Instrument Bypass", None, Global;
    InstrumentOffline, "instrument.offline", "Toggle Instrument Offline", None, Global;
    CreateClip, "clip.create", "Create Clip", plain(Key::Enter), Arrangement;
    SelectAllClips, "clip.select-all", "Select All Clips", cmd(Key::A), Arrangement;
    DuplicateClips, "clip.duplicate", "Duplicate", cmd(Key::D), Arrangement;
    DeleteClips, "clip.delete", "Delete", plain(Key::Backspace), Arrangement;
    SplitClips, "clip.split", "Split at Playhead", cmd(Key::E), Arrangement;
    MoveClipsLeft, "clip.earlier", "Move Clips Earlier", key(Modifiers::ALT, Key::ArrowLeft), Arrangement;
    MoveClipsRight, "clip.later", "Move Clips Later", key(Modifiers::ALT, Key::ArrowRight), Arrangement;
    MoveClipsUp, "clip.up", "Move Clips Up a Track", key(Modifiers::ALT, Key::ArrowUp), Arrangement;
    MoveClipsDown, "clip.down", "Move Clips Down a Track", key(Modifiers::ALT, Key::ArrowDown), Arrangement;
    ResizeClipShorter, "clip.shorten", "Shorten Clip", key(ALT_SHIFT, Key::ArrowLeft), Arrangement;
    ResizeClipLonger, "clip.lengthen", "Lengthen Clip", key(ALT_SHIFT, Key::ArrowRight), Arrangement;
    LoopClipShorter, "clip.loop-shorten", "Shorten Clip Loop", key(CMD_ALT, Key::ArrowLeft), Arrangement;
    LoopClipLonger, "clip.loop-extend", "Extend Clip Loop", key(CMD_ALT, Key::ArrowRight), Arrangement;
    OpenRecent, "startup.open", "Open", None, Global;
    LocateRecent, "startup.locate", "Locate…", None, Global;
    RemoveRecent, "startup.remove", "Remove", None, Global;
    ShowRecovery, "startup.recovery", "Recovery…", None, Global;
    RecoverProject, "startup.recover", "Recover", None, Global;
    DiscardRecovery, "startup.discard", "Discard…", None, Global;
    RecoveryLater, "startup.later", "Later", None, Global;
    Settings, "window.settings", "Audio/MIDI Settings", None, Global;
    TrackDiagnostics, "window.track-diagnostics", "Track Diagnostics", None, Global;
    Commands, "window.commands", "Commands / Keyboard Shortcuts", cmd(Key::K), Global;
    Palette, "window.palette", "Command Palette…", key(CMD_SHIFT, Key::P), Global;
    AddTrack, "track.add", "Add Instrument Track", None, Global;
    FitArrangementWidth, "view.fit-arrangement-width", "Fit Arrangement to Width", None, Global;
    FitArrangementHeight, "view.fit-arrangement-height", "Fit Arrangement to Height", None, Global;
    FitRollWidth, "view.fit-roll-width", "Fit Piano Roll to Width", None, Global;
    FitRollHeight, "view.fit-roll-height", "Fit Piano Roll to Height", None, Global;
    AddBus, "track.add-bus", "Add Bus", None, Global;
    Instrument, "track.instrument", "Choose Instrument", None, Global;
    Editor, "track.editor", "Open Editor", None, Global;
    CloseEditor, "track.close-editor", "Close Editor", None, Global;
    Restart, "track.restart", "Restart Instrument", None, Global;
    Rescan, "plugins.rescan", "Rescan Plugins", None, Global;
    Refresh, "devices.refresh", "Refresh Devices", None, Global;
    Rename, "track.rename", "Rename Track", None, Global;
    ApplySettings, "settings.apply", "Apply Audio/MIDI Settings", None, Global;
    RemoveTrack, "track.remove", "Remove Track…", None, Global;
    MoveUp, "track.up", "Move Track Up", None, Global;
    MoveDown, "track.down", "Move Track Down", None, Global;
    PreviousTrack, "track.previous", "Select Previous Track", None, Global;
    NextTrack, "track.next", "Select Next Track", None, Global;
    Quarantine, "plugins.quarantine", "Show Quarantined Plugins…", None, Global;
    Reactivate, "plugins.reactivate", "Reactivate Plugin…", None, Global;
    Play, "transport.play", "Play", None, Global;
    Record, "transport.record", "Record MIDI", None, Global;
    ArmTrack, "track.arm", "Toggle Record Arm for Selected Track", None, Global;
    RecordMerge, "transport.record-merge", "Record Mode: Merge", None, Global;
    RecordReplace, "transport.record-replace", "Record Mode: Replace", None, Global;
    RecordNewTake, "transport.record-new-take", "Record Mode: New take", None, Global;
    RecordTrackGlobal, "track.record-global", "Track Record Mode: Global", None, Global;
    RecordTrackMerge, "track.record-merge", "Track Record Mode: Merge", None, Global;
    RecordTrackReplace, "track.record-replace", "Track Record Mode: Replace", None, Global;
    RecordTrackNewTake, "track.record-new-take", "Track Record Mode: New take", None, Global;
    ShowTakeLanes, "track.take-lanes", "Show/Hide Take Lanes", None, Global;
    UseWholeTake, "take.use-whole", "Use Whole Take", None, Arrangement;
    DeleteTake, "take.delete", "Delete Take", None, Arrangement;
    FlattenComp, "take.flatten", "Flatten Comp", None, Arrangement;
    TakeToNewTrack, "take.to-new-track", "Take to New Track", None, Arrangement;
    RenameTake, "take.rename", "Rename Take", None, Arrangement;
    EditTake, "take.edit", "Edit Take", None, Arrangement;
    Stop, "transport.stop", "Stop", None, Global;
    PlayStop, "transport.play-stop", "Play / Stop", plain(Key::Space), Global;
    ReturnToStart, "transport.return", "Return to Start", None, Global;
    ToggleLoop, "transport.loop", "Loop", None, Global;
    ToggleClick, "transport.click", "Metronome", None, Global;
    ToggleCountIn, "transport.count-in", "One-Bar Count-In", None, Global;
    TempoMap, "transport.tempo-map", "Tempo and Meter Map…", None, Global;
    TransportSettings, "transport.settings", "Transport Settings…", None, Global;
    KeepStopPosition, "transport.stop-keep", "Stop Mode: Keep Position", None, Global;
    ReturnStopPosition, "transport.stop-return", "Stop Mode: Return to Play Start", None, Global;
    RollSelectAll, "roll.select-all", "Piano Roll: Select All Events", cmd(Key::A), PianoRoll;
    RollDelete, "roll.delete", "Piano Roll: Delete Events", plain(Key::Backspace), PianoRoll;
    RollCopy, "roll.copy", "Piano Roll: Copy Events", cmd(Key::C), PianoRoll;
    RollPaste, "roll.paste", "Piano Roll: Paste Events at Playhead", cmd(Key::V), PianoRoll;
    RollDuplicate, "roll.duplicate", "Piano Roll: Duplicate Events", cmd(Key::D), PianoRoll;
    RollEnter, "roll.enter", "Piano Roll: Add Note / Audition Focused Note", plain(Key::Enter), PianoRoll;
    RollEarlier, "roll.earlier", "Piano Roll: Move Events Earlier", plain(Key::ArrowLeft), PianoRoll;
    RollLater, "roll.later", "Piano Roll: Move Events Later", plain(Key::ArrowRight), PianoRoll;
    RollUp, "roll.up", "Piano Roll: Move Up / Increase Value", plain(Key::ArrowUp), PianoRoll;
    RollDown, "roll.down", "Piano Roll: Move Down / Decrease Value", plain(Key::ArrowDown), PianoRoll;
    RollShorten, "roll.shorten", "Piano Roll: Shorten Notes", key(Modifiers::SHIFT, Key::ArrowLeft), PianoRoll;
    RollLengthen, "roll.lengthen", "Piano Roll: Lengthen Notes", key(Modifiers::SHIFT, Key::ArrowRight), PianoRoll;
}

impl Command {
    pub(crate) fn spec(self) -> &'static Spec {
        SPECS
            .iter()
            .find(|spec| spec.command == self)
            .expect("every command has a registry entry")
    }

    pub(crate) fn title(self) -> &'static str {
        self.spec().title
    }

    pub(crate) fn context(self) -> Context {
        self.spec().context
    }

    pub(crate) fn is_clip(self) -> bool {
        use Command::*;
        matches!(
            self,
            CreateClip
                | SelectAllClips
                | DuplicateClips
                | DeleteClips
                | SplitClips
                | MoveClipsLeft
                | MoveClipsRight
                | MoveClipsUp
                | MoveClipsDown
                | ResizeClipShorter
                | ResizeClipLonger
                | LoopClipShorter
                | LoopClipLonger
        )
    }

    /// Commands where Option changes how the action behaves (snap off) rather than which
    /// command runs: their binding matches with or without Option held.
    fn option_free(self) -> bool {
        use Command::*;
        matches!(
            self,
            RollEnter | RollEarlier | RollLater | RollUp | RollDown | RollShorten | RollLengthen
        )
    }

    /// Destructive commands paint in `danger` in menus.
    pub(crate) fn destructive(self) -> bool {
        use Command::*;
        matches!(
            self,
            DeleteClips
                | DeleteTake
                | RemoveTrack
                | RemoveRecent
                | DiscardRecovery
                | InsertRemove
                | RollDelete
                | FlattenComp
        )
    }
}

/// The live shortcuts: the registry defaults with the user's saved changes applied.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Keymap {
    shortcuts: Vec<Option<KeyboardShortcut>>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self {
            shortcuts: SPECS.iter().map(|spec| spec.default).collect(),
        }
    }
}

impl Keymap {
    fn index(command: Command) -> usize {
        SPECS
            .iter()
            .position(|spec| spec.command == command)
            .expect("every command has a registry entry")
    }

    pub(crate) fn shortcut(&self, command: Command) -> Option<KeyboardShortcut> {
        self.shortcuts[Self::index(command)]
    }

    /// The live shortcut as macOS shows it ("⇧⌘Z"), empty when unbound.
    pub(crate) fn text(&self, ctx: &egui::Context, command: Command) -> String {
        self.shortcut(command)
            .map(|shortcut| ctx.format_shortcut(&shortcut))
            .unwrap_or_default()
    }

    /// "Title (⌘S)" for tooltips, or the title alone when unbound.
    pub(crate) fn tooltip(&self, ctx: &egui::Context, command: Command) -> String {
        let shortcut = self.text(ctx, command);
        if shortcut.is_empty() {
            command.title().into()
        } else {
            format!("{} ({shortcut})", command.title())
        }
    }

    pub(crate) fn set(&mut self, command: Command, shortcut: Option<KeyboardShortcut>) {
        self.shortcuts[Self::index(command)] = shortcut;
    }

    /// The command whose binding would clash with `shortcut` on `command`, if any.
    pub(crate) fn conflict(&self, command: Command, shortcut: KeyboardShortcut) -> Option<Command> {
        let context = command.context();
        SPECS
            .iter()
            .zip(&self.shortcuts)
            .find(|(spec, bound)| {
                spec.command != command
                    && **bound == Some(shortcut)
                    && spec.context.overlaps(context)
            })
            .map(|(spec, _)| spec.command)
    }

    /// True when `shortcut` belongs to `command` and is pressed: the key event is removed so
    /// no other handler sees it. Unmodified Backspace bindings also accept Delete.
    pub(crate) fn consume(&self, input: &mut InputState, command: Command) -> bool {
        let Some(shortcut) = self.shortcut(command) else {
            return false;
        };
        let option_free = command.option_free();
        let mut consumed = false;
        input.events.retain(|event| {
            let matched = !consumed
                && matches!(event, egui::Event::Key { key, modifiers, pressed: true, .. }
                if (*key == shortcut.logical_key
                    || shortcut.logical_key == Key::Backspace
                        && shortcut.modifiers.is_none()
                        && *key == Key::Delete)
                    && modifiers.matches_exact(Modifiers {
                        alt: if option_free { modifiers.alt } else { shortcut.modifiers.alt },
                        ..shortcut.modifiers
                    }));
            consumed |= matched;
            !matched
        });
        consumed
    }

    /// The saved form: only the bindings that differ from the defaults, by stable id;
    /// `None` records an explicitly cleared default.
    pub(crate) fn overrides(&self) -> BTreeMap<String, Option<String>> {
        SPECS
            .iter()
            .zip(&self.shortcuts)
            .filter(|(spec, bound)| spec.default != **bound)
            .map(|(spec, bound)| (spec.id.to_owned(), bound.map(encode)))
            .collect()
    }

    /// The defaults with `saved` applied. Unknown ids, unreadable shortcuts and entries that
    /// would clash with an earlier binding are skipped and reported.
    pub(crate) fn from_overrides(saved: &BTreeMap<String, Option<String>>) -> (Self, Vec<String>) {
        let mut keymap = Self::default();
        let mut problems = Vec::new();
        for (id, value) in saved {
            let Some(spec) = SPECS.iter().find(|spec| spec.id == id) else {
                problems.push(format!("unknown command {id}"));
                continue;
            };
            let shortcut = match value.as_deref().map(decode) {
                None => None,
                Some(Some(shortcut)) => Some(shortcut),
                Some(None) => {
                    problems.push(format!("unreadable shortcut for {id}"));
                    continue;
                }
            };
            keymap.set(spec.command, shortcut);
        }
        // A saved file may still clash with a default it did not mention: the default loses.
        for spec in SPECS {
            if let Some(shortcut) = keymap.shortcut(spec.command)
                && saved.contains_key(spec.id)
                && let Some(other) = keymap.conflict(spec.command, shortcut)
            {
                if saved.contains_key(other.spec().id) {
                    problems.push(format!("{} clashes with {}", spec.id, other.spec().id));
                    keymap.set(spec.command, spec.default);
                } else {
                    keymap.set(other, None);
                }
            }
        }
        (keymap, problems)
    }
}

/// A key pressed while assigning a shortcut, with platform modifiers normalised so that ⌘ on
/// macOS and Ctrl elsewhere both become `COMMAND`.
pub(crate) fn captured(key: Key, modifiers: Modifiers) -> KeyboardShortcut {
    KeyboardShortcut::new(
        Modifiers {
            alt: modifiers.alt,
            shift: modifiers.shift,
            ctrl: modifiers.ctrl && !(modifiers.command && !modifiers.mac_cmd),
            command: modifiers.command,
            mac_cmd: false,
        },
        key,
    )
}

/// True when the shortcut has no Command, Control or Option: it never fires while text is
/// being typed.
pub(crate) fn unmodified(shortcut: KeyboardShortcut) -> bool {
    let m = shortcut.modifiers;
    !(m.command || m.ctrl || m.alt || m.mac_cmd)
}

fn encode(shortcut: KeyboardShortcut) -> String {
    let m = shortcut.modifiers;
    let mut parts = Vec::new();
    for (on, name) in [
        (m.ctrl, "Ctrl"),
        (m.alt, "Alt"),
        (m.shift, "Shift"),
        (m.command || m.mac_cmd, "Cmd"),
    ] {
        if on {
            parts.push(name);
        }
    }
    parts.push(shortcut.logical_key.name());
    parts.join("+")
}

fn decode(text: &str) -> Option<KeyboardShortcut> {
    let mut parts: Vec<&str> = text.split('+').collect();
    let key = Key::from_name(parts.pop()?)?;
    let mut modifiers = Modifiers::NONE;
    for part in parts {
        match part {
            "Ctrl" => modifiers.ctrl = true,
            "Alt" => modifiers.alt = true,
            "Shift" => modifiers.shift = true,
            "Cmd" => modifiers.command = true,
            _ => return None,
        }
    }
    Some(KeyboardShortcut::new(modifiers, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_commands_are_unique_and_defaults_do_not_clash() {
        for (index, spec) in SPECS.iter().enumerate() {
            assert!(
                SPECS[..index].iter().all(|other| other.id != spec.id),
                "duplicate id {}",
                spec.id
            );
            assert!(
                SPECS[..index]
                    .iter()
                    .all(|other| other.command != spec.command),
                "duplicate command {:?}",
                spec.command
            );
        }
        let keymap = Keymap::default();
        for spec in SPECS {
            if let Some(shortcut) = spec.default {
                assert_eq!(
                    keymap.conflict(spec.command, shortcut),
                    None,
                    "{} clashes",
                    spec.id
                );
            }
        }
    }

    #[test]
    fn contexts_overlap_only_with_global_and_themselves() {
        use Context::*;
        assert!(Global.overlaps(PianoRoll) && PianoRoll.overlaps(Global));
        assert!(Arrangement.overlaps(Arrangement));
        assert!(!Arrangement.overlaps(PianoRoll) && !Mixer.overlaps(Arrangement));
        let keymap = Keymap::default();
        // ⌘A belongs to both the arrangement and the roll without clashing,
        assert_eq!(
            keymap.conflict(
                Command::RollSelectAll,
                KeyboardShortcut::new(Modifiers::COMMAND, Key::A)
            ),
            None
        );
        // but a global binding clashes with a roll binding.
        assert_eq!(
            keymap.conflict(
                Command::New,
                KeyboardShortcut::new(Modifiers::COMMAND, Key::A)
            ),
            Some(Command::SelectAllClips)
        );
    }

    #[test]
    fn shortcuts_round_trip_through_the_saved_form() {
        for spec in SPECS {
            if let Some(shortcut) = spec.default {
                assert_eq!(decode(&encode(shortcut)), Some(shortcut), "{}", spec.id);
            }
        }
        let mut keymap = Keymap::default();
        keymap.set(Command::Save, None);
        keymap.set(
            Command::Play,
            Some(KeyboardShortcut::new(Modifiers::NONE, Key::P)),
        );
        let saved = keymap.overrides();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved["file.save"], None);
        assert_eq!(saved["transport.play"].as_deref(), Some("P"));
        let (loaded, problems) = Keymap::from_overrides(&saved);
        assert!(problems.is_empty());
        assert_eq!(loaded.shortcut(Command::Save), None);
        assert_eq!(loaded.overrides(), saved);
    }

    #[test]
    fn a_partial_or_bad_saved_form_keeps_the_defaults_and_reports() {
        let saved = BTreeMap::from([
            ("no.such".to_owned(), Some("Cmd+X".to_owned())),
            ("file.save".to_owned(), Some("Cmd+Nope".to_owned())),
            // Takes ⌘N from New, which loses its default.
            ("transport.play".to_owned(), Some("Cmd+N".to_owned())),
        ]);
        let (keymap, problems) = Keymap::from_overrides(&saved);
        assert_eq!(problems.len(), 2);
        assert_eq!(keymap.shortcut(Command::Save), Command::Save.spec().default);
        assert_eq!(keymap.shortcut(Command::New), None);
        assert_eq!(
            keymap.shortcut(Command::Play),
            Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::N))
        );
    }

    #[test]
    fn matching_is_exact_except_option_on_roll_steps_and_delete_for_backspace() {
        let keymap = Keymap::default();
        let ctx = egui::Context::default();
        let press = |key, modifiers, command| {
            let mut fired = false;
            let _ = ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ui| fired = ui.input_mut(|input| keymap.consume(input, command)),
            );
            fired
        };
        let mac_cmd = Modifiers::MAC_CMD | Modifiers::COMMAND;
        assert!(press(Key::Z, mac_cmd, Command::Undo));
        assert!(!press(Key::Z, mac_cmd | Modifiers::SHIFT, Command::Undo));
        assert!(press(Key::ArrowLeft, Modifiers::ALT, Command::RollEarlier));
        assert!(!press(
            Key::ArrowLeft,
            Modifiers::SHIFT,
            Command::RollEarlier
        ));
        assert!(!press(
            Key::ArrowLeft,
            Modifiers::NONE,
            Command::MoveClipsLeft
        ));
        assert!(press(Key::Delete, Modifiers::NONE, Command::RollDelete));
        assert_eq!(
            captured(Key::K, mac_cmd),
            KeyboardShortcut::new(Modifiers::COMMAND, Key::K)
        );
        assert_eq!(
            captured(Key::K, Modifiers::CTRL),
            KeyboardShortcut::new(Modifiers::CTRL, Key::K)
        );
    }
}
