---
name: Notice box
kind: component
reference: composer::style::NoticeBox, composer::style::NoticeContent
status: shared
impl: gallery
tokens: [panel, divider, text, secondary, danger, peacock]
used-in: [quarantine-notice, arrangement, ara-editor]
snapshot: snapshots/egui/t18-notice-box-1024.png
api: ui::primitives::notice_box::NoticeBox
evidence: [t18-notice-box, t18-notice-question]
---

> **Migrated to Rust/egui (T18, 2026-11-21).** The Clef section at the end of this file is the
> implementation contract. Earlier sections keep the legacy design intent; their C++ symbols are
> not in this repository.

# Notice box

## Purpose
The notice shown once over the workspace when a project opens with something the composer must know about:
quarantined plugins (D88) and missing audio files (D05), and when the disk stopped a recording (D106). It is not a
general dialog.

## Anatomy
A solid `panel` box, 560 wide and centred, with a 1-pixel `divider` edge and no shadow, padded 16: the
`warning` glyph (16, `danger`) and the heading at 16 bold; two lines of `secondary` body text; 11-px uppercase
column captions; one 22-pixel row per entry (first column `danger`, second `text`, third `secondary`), at most
twelve, then "… and N more"; bottom right a secondary text button and the primary `peacock` button.

## Question form (D87)
`NoticeContent::question = true`: no warning glyph, no columns and no rows; the heading, two lines of `secondary`
text and the two buttons. Used only for a question that stands between a command and its effect ("Render ARA
edits first?", snapshot `snapshots/ara-render-question.png`); the primary button names what will happen
("Render and Reverse ↩").

## Stopped form (D106)
A take the disk stopped (it could not keep up, filled or failed) is already committed, so the box has nothing to choose:
`secondary` left empty hides that button, and the primary "OK ↩" only closes it. Heading "Recording stopped at bar N",
the cause and "The take is in the project, one Undo step as usual." as body, columns Track · File · Kept.

## Behaviour
Return runs the primary button; Escape, a click outside the box or the secondary button closes it (the
secondary button runs its action first). While it shows it takes the keyboard. The owner keeps the one-line
form in the status row.

## Clef Rust/egui — T18

**API**

```rust
impl<'a> NoticeBox<'a> {
    pub fn new(id: &'a str, heading: &'a str, body: &'a str, primary: &'a str) -> Self;
    pub fn question(self) -> Self;                 // no glyph, no rows (D87)
    pub fn rows(self, columns: &'a [&'a str], rows: &'a [[String; 3]]) -> Self;
    pub fn secondary(self, label: &'a str) -> Self; // omitted: stopped form (D106)
    pub fn show(self, ctx: &egui::Context) -> Option<Outcome>; // Primary | Secondary | Dismissed
}
```

An `egui::Modal` with a `PANEL` frame, 1-point `DIVIDER` edge, 4-point corners, padded 16, at most
560 wide. Warning glyph 16 in `DANGER`, `heading` text, body in `SECONDARY`, `section` column
captions, up to twelve rows (`DANGER` / `TEXT` / `SECONDARY`) then "… and N more", buttons right
aligned with the primary [text button](text-button.md) last. Return chooses the primary button;
Escape or a click outside returns `Dismissed`. Used by the quarantine notice; the
[question form](../snapshots/egui/t18-notice-question-1024.png) is ready for the ARA render question.
