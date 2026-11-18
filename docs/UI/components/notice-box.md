---
name: Notice box
kind: component
reference: composer::style::NoticeBox, composer::style::NoticeContent
status: shared
impl: planned
tokens: [panel, divider, text, secondary, danger, peacock]
used-in: [quarantine-notice, arrangement, ara-editor]
snapshot: snapshots/audio-missing-notice.png
---

> **Legacy reference (C++).** The symbols and code here describe the original Composer
> implementation; they are not in this repository and are not an API. Clef builds this in
> Rust + egui. See [README](../README.md) and the [UI contract](../contract.md).

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

## Code
```cpp
style::NoticeContent content;
content.heading = "1 audio file is missing";
content.captions = { "File", "Parts", "Last location" };
content.rows.push_back({ "Room take 2.wav", "Room mics · 1 part", "Audio/ in this project" });
content.secondary = "Later"; content.primary = juce::String::fromUTF8("Relink… ↩");
auto box = std::make_unique<style::NoticeBox>(content, nullptr, relink, close);
```
